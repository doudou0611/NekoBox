use super::*;
use crate::domain::requests::ConfirmMetadataMatchRequest;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub status: String,
    pub total: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub current: Option<String>,
    pub messages: Vec<String>,
}
#[derive(Default)]
pub struct Manager {
    pub state: Status,
    pub cancel: Arc<AtomicBool>,
}
#[derive(Deserialize)]
pub struct Request {
    pub confirmed: bool,
}
pub fn status(b: &Backend) -> Result<Status> {
    Ok(b.metadata_refresh
        .lock()
        .map_err(|_| invalid("更新服务不可用。"))?
        .state
        .clone())
}
pub fn cancel(b: &Backend) -> Result<bool> {
    b.metadata_refresh
        .lock()
        .map_err(|_| invalid("更新服务不可用。"))?
        .cancel
        .store(true, Ordering::Relaxed);
    Ok(true)
}
pub fn start(b: &Backend, q: Request) -> Result<Status> {
    if !q.confirmed {
        return Err(invalid("全库更新需要确认。"));
    }
    let config = metadata_sources::get(b)?;
    let prefs = app_settings::get(b)?;
    let translation = translation::configuration(b)?;
    let ids = b.database()?.refresh_game_ids()?;
    let mut manager = b
        .metadata_refresh
        .lock()
        .map_err(|_| invalid("更新服务不可用。"))?;
    if manager.state.status == "running" {
        return Err(invalid("已有元数据更新正在进行。"));
    }
    manager.cancel = Arc::new(AtomicBool::new(false));
    let cancelled = manager.cancel.clone();
    manager.state = Status {
        status: "running".into(),
        total: ids.len(),
        ..Status::default()
    };
    let initial = manager.state.clone();
    drop(manager);
    let mut b = b.clone();
    b.metadata_snapshot = Some(config.clone());
    b.app_settings_snapshot = Some(prefs);
    b.translation_snapshot = Some(translation.clone());
    std::thread::spawn(move || {
        for game in ids {
            if cancelled.load(Ordering::Relaxed) {
                break;
            }
            let result = (|| -> Result<bool> {
                let detail = b.database()?.get_game(&game)?;
                if detail.metadata_locked {
                    return Ok(false);
                }
                if let Ok(mut m) = b.metadata_refresh.lock() {
                    m.state.current = Some(detail.summary.title.clone());
                }
                let steam_id = detail
                    .metadata
                    .iter()
                    .find(|field| field.provider == "steam" && field.field == "title")
                    .and_then(|field| field.remote_id.clone())
                    .or_else(|| {
                        detail
                            .summary
                            .installations
                            .iter()
                            .find(|i| {
                                matches!(i.source, crate::domain::protocol::InstallSource::Steam)
                            })
                            .and_then(|i| i.steam_app_id.clone())
                    });
                if let Some(steam_id) = steam_id {
                    let expected = serde_json::to_string(&detail.metadata)
                        .map_err(|_| invalid("资料状态无效。"))?;
                    let mut stage = b.clone();
                    let mut db = Database::in_memory()?;
                    db.restore_verified_snapshot(&*b.database()?)?;
                    stage.db = Arc::new(Mutex::new(db));
                    let community = detail
                        .metadata
                        .iter()
                        .find(|f| f.provider == "hikarinagi" && f.field == "community_binding")
                        .and_then(|f| f.remote_id.as_deref());
                    super::steam::confirm(
                        &stage,
                        &ConfirmMetadataMatchRequest {
                            manual: false,
                            title_hint: None,
                            game_id: game.clone(),
                            provider: "steam".into(),
                            remote_id: steam_id,
                        },
                        community,
                        community.is_some(),
                    )?;
                    if cancelled.load(Ordering::Relaxed) {
                        return Err(ServiceError(ErrorCode::Cancelled, "更新已取消。"));
                    }
                    b.database()?.commit_metadata_refresh(
                        &game,
                        &expected,
                        &*stage.database()?,
                        &["steam".into()],
                    )?;
                    return Ok(true);
                }
                let bindings = b
                    .database()?
                    .list_external_sources(&game)?
                    .into_iter()
                    .filter(|s| config.enabled().contains(&s.provider))
                    .filter_map(|s| Some((s.provider, s.remote_id?)))
                    .collect::<std::collections::BTreeMap<_, _>>();
                if bindings.is_empty() {
                    return Ok(false);
                }
                let expected =
                    serde_json::to_string(&detail.metadata).map_err(|_| invalid("资料无效。"))?;
                let mut stage = b.clone();
                let mut db = Database::in_memory()?;
                db.restore_verified_snapshot(&*b.database()?)?;
                db.clear_refresh_cache(&game)?;
                db.save_translation_config(&translation.0)?;
                stage.db = Arc::new(Mutex::new(db));
                stage
                    .database()?
                    .set_metadata_priority(&game, &config.enabled())?;
                let mut last = None;
                for provider in config.enabled() {
                    if cancelled.load(Ordering::Relaxed) {
                        return Err(ServiceError(ErrorCode::Cancelled, "更新已取消。"));
                    }
                    let Some(remote) = bindings.get(&provider) else {
                        continue;
                    };
                    let q = ConfirmMetadataMatchRequest {
                        manual: false,
                        title_hint: None,
                        game_id: game.clone(),
                        provider: provider.clone(),
                        remote_id: remote.clone(),
                    };
                    let matched = match provider.as_str() {
                        "bangumi" => bangumi::confirm_metadata(&stage, &q),
                        "vndb" => vndb::confirm(&stage, &q),
                        _ => hikarinagi::confirm(&stage, &q),
                    }?;
                    last = Some((q, matched));
                }
                let cap = app_settings::get(&stage)?.tag_limit;
                stage.database()?.limit_source_tags(&game, cap)?;
                if let Some((q, mut matched)) = last {
                    translation::complete(&stage, &q, &mut matched);
                    if let Some(message) = matched.translation_message {
                        if let Ok(mut manager) = b.metadata_refresh.lock() {
                            if manager.state.messages.len() < 100 {
                                manager
                                    .state
                                    .messages
                                    .push(format!("{}：{}", detail.summary.title, message));
                            }
                        }
                    }
                }
                if cancelled.load(Ordering::Relaxed) {
                    return Err(ServiceError(ErrorCode::Cancelled, "更新已取消。"));
                }
                b.database()?.commit_metadata_refresh(
                    &game,
                    &expected,
                    &*stage.database()?,
                    &config.enabled(),
                )?;
                Ok(true)
            })();
            if let Ok(mut m) = b.metadata_refresh.lock() {
                match result {
                    Ok(true) => m.state.succeeded += 1,
                    Ok(false) => m.state.skipped += 1,
                    Err(e) if e.0 == ErrorCode::Cancelled => break,
                    Err(e) => {
                        m.state.failed += 1;
                        if m.state.messages.len() < 100 {
                            let current = m.state.current.clone().unwrap_or_else(|| "作品".into());
                            m.state.messages.push(format!("{current}：{}", e.1));
                        }
                    }
                }
                m.state.processed += 1;
            }
        }
        if let Ok(mut m) = b.metadata_refresh.lock() {
            m.state.status = if cancelled.load(Ordering::Relaxed) {
                "cancelled"
            } else {
                "completed"
            }
            .into();
            m.state.current = None;
        }
    });
    Ok(initial)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unbound_games_are_skipped_without_guessing_a_remote_match() {
        let root = std::env::temp_dir().join(format!("gm-refresh-{}", id()));
        let b = Backend::open(root.clone()).unwrap();
        let game = b
            .database()
            .unwrap()
            .import_installation(
                "C:/missing/game",
                "Original",
                None,
                crate::domain::protocol::InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        assert!(start(&b, Request { confirmed: false }).is_err());
        assert_eq!(start(&b, Request { confirmed: true }).unwrap().total, 1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let state = status(&b).unwrap();
            if state.status != "running" {
                assert_eq!(state.status, "completed");
                assert_eq!(state.processed, 1);
                assert_eq!(state.skipped, 1);
                assert_eq!(state.succeeded, 0);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            b.database().unwrap().get_game(&game).unwrap().summary.title,
            "Original"
        );
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
}

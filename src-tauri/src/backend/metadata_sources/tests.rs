use super::*;
use std::{cell::RefCell, fs};
struct Fixture {
    root: PathBuf,
    b: Backend,
    game: String,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(id());
        let b = Backend::open(root.join("data")).unwrap();
        let game = b
            .database()
            .unwrap()
            .import_installation(
                "/fixture/game",
                "game",
                None,
                crate::domain::protocol::InstallSource::Manual,
                &[],
                "",
            )
            .unwrap();
        Self { root, b, game }
    }
    fn request(&self) -> ConfirmMetadataMatchRequest {
        ConfirmMetadataMatchRequest {
            manual: false,
            title_hint: None,
            game_id: self.game.clone(),
            provider: "hikarinagi".into(),
            remote_id: "1".into(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn apply_fields(
    b: &Backend,
    q: &ConfirmMetadataMatchRequest,
    chinese: bool,
) -> Result<MatchResult> {
    b.database()?.apply_remote_fields(
        &q.game_id,
        &q.provider,
        &q.remote_id,
        &[
            ("title".into(), "Shared Title".into()),
            (
                "description".into(),
                if chinese {
                    "这是来源维护的中文简介，人物名字也保留中文。"
                } else {
                    "This is an English source summary."
                }
                .into(),
            ),
            (
                "publisher".into(),
                if chinese {
                    "中文发行商"
                } else {
                    "Foreign Publisher"
                }
                .into(),
            ),
        ],
        &[],
        &now(),
        true,
    )
}
fn candidate(provider: &str, remote: &str) -> MetadataCandidate {
    MetadataCandidate {
        provider: provider.into(),
        remote_id: remote.into(),
        title: "Shared Title".into(),
        subtitle: None,
        cover_url: None,
        has_chinese_description: None,
        confidence: 1.0,
        matched_fields: vec![],
        explanation: String::new(),
        fetched_at: now(),
        cached: false,
    }
}
// Multi-source scenarios explicitly opt in instead of depending on production defaults.
fn all_sources() -> Config {
    let mut config = Config::default();
    for source in &mut config.sources {
        source.enabled = true;
    }
    config
}
#[test]
fn default_only_enables_hikarinagi_and_saved_preferences_survive_reopen() {
    let root = std::env::temp_dir().join(id());
    let b = Backend::open(root.join("data")).unwrap();
    let default = get(&b).unwrap();
    assert_eq!(default.enabled(), vec!["hikarinagi"]);
    assert_eq!(
        default
            .sources
            .iter()
            .map(|s| s.provider.as_str())
            .collect::<Vec<_>>(),
        vec!["hikarinagi", "bangumi", "vndb"]
    );
    assert!(b
        .database()
        .unwrap()
        .setting::<Config>(SETTING)
        .unwrap()
        .is_none());
    let mut saved = all_sources();
    saved.sources.reverse();
    saved.sources[2].enabled = false;
    save(&b, saved.clone()).unwrap();
    drop(b);
    let reopened = Backend::open(root.join("data")).unwrap();
    assert_eq!(get(&reopened).unwrap().enabled(), vec!["vndb", "bangumi"]);
    assert_eq!(
        serde_json::to_value(get(&reopened).unwrap()).unwrap(),
        serde_json::to_value(saved).unwrap()
    );
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn config_requires_exact_providers_and_one_enabled() {
    let mut c = Config::default();
    assert_eq!(c.enabled(), vec!["hikarinagi"]);
    for s in &mut c.sources {
        s.enabled = false;
    }
    assert!(c.validate().is_err());
    c.sources[0].enabled = true;
    assert!(c.validate().is_ok());
    c.sources[1].provider = "hikarinagi".into();
    assert!(c.validate().is_err());
}
#[test]
fn supplementation_respects_disabled_source_snapshot_and_order_over_language() {
    let f = Fixture::new();
    let mut config = all_sources();
    config.sources[2].enabled = false;
    save(&f.b, config).unwrap();
    let calls = RefCell::new(vec![]);
    confirm_with(
        &f.b,
        &f.request(),
        |b, q| {
            calls.borrow_mut().push(q.provider.clone());
            let r = apply_fields(b, q, q.provider == "bangumi")?;
            if q.provider == "hikarinagi" {
                let mut changed = all_sources();
                changed.sources[0].enabled = false;
                changed.sources[1].enabled = false;
                save(&f.b, changed)?;
            }
            Ok(r)
        },
        |b, q| {
            assert_eq!(get(b)?.enabled(), vec!["hikarinagi", "bangumi"]);
            Ok(vec![candidate(&q.providers[0], "2")])
        },
        true,
    )
    .unwrap();
    assert_eq!(*calls.borrow(), vec!["hikarinagi", "bangumi"]);
    let detail = f.b.database().unwrap().get_game(&f.game).unwrap();
    assert!(detail.description.unwrap().starts_with("This is"));
    assert_eq!(
        detail.summary.publisher.as_deref(),
        Some("Foreign Publisher")
    );
    assert_eq!(get(&f.b).unwrap().enabled(), vec!["vndb"]);
}
#[test]
fn an_old_binding_does_not_hide_a_new_fetch_failure() {
    let f = Fixture::new();
    apply_fields(&f.b, &f.request(), false).unwrap();
    let result = confirm_with(
        &f.b,
        &f.request(),
        |_, _| {
            Err(ServiceError(
                ErrorCode::NetworkUnavailable,
                "本轮请求失败。",
            ))
        },
        |_, _| Ok(vec![]),
        true,
    );
    assert!(result.is_err());
}
#[test]
fn ambiguous_same_title_candidates_never_bind_another_work() {
    let f = Fixture::new();
    save(&f.b, all_sources()).unwrap();
    let calls = RefCell::new(vec![]);
    let result = confirm_with(
        &f.b,
        &f.request(),
        |b, q| {
            calls.borrow_mut().push(q.provider.clone());
            apply_fields(b, q, false)
        },
        |_, q| {
            Ok(vec![
                candidate(&q.providers[0], "2"),
                candidate(&q.providers[0], "3"),
            ])
        },
        true,
    )
    .unwrap();
    assert_eq!(*calls.borrow(), vec!["hikarinagi"]);
    assert!(result.supplementation_message.unwrap().contains("唯一"));
}
#[test]
fn partial_cover_failure_keeps_fresh_metadata_and_can_use_next_source_cover() {
    let f = Fixture::new();
    let mut config = all_sources();
    config.sources[2].enabled = false;
    save(&f.b, config).unwrap();
    let result = confirm_with(
        &f.b,
        &f.request(),
        |b, q| {
            let r = apply_fields(b, q, q.provider == "bangumi")?;
            if q.provider == "hikarinagi" {
                return Err(ServiceError(
                    ErrorCode::NetworkUnavailable,
                    "封面下载失败。",
                ));
            }
            b.database()?.apply_remote_fields(
                &q.game_id,
                &q.provider,
                &q.remote_id,
                &[("cover_path".into(), {
                    use sha2::{Digest, Sha256};
                    let mut encoded = std::io::Cursor::new(Vec::new());
                    image::DynamicImage::new_rgb8(2, 2)
                        .write_to(&mut encoded, image::ImageFormat::Png)
                        .unwrap();
                    let bytes = encoded.into_inner();
                    let name = format!("{:x}.png", Sha256::digest(&bytes));
                    fs::create_dir_all(b.data_directory.join("covers")).unwrap();
                    fs::write(b.data_directory.join("covers").join(&name), bytes).unwrap();
                    format!("covers/{name}")
                })],
                &[],
                &now(),
                false,
            )?;
            Ok(r)
        },
        |_, q| Ok(vec![candidate(&q.providers[0], "2")]),
        true,
    )
    .unwrap();
    assert!(result
        .supplementation_message
        .unwrap()
        .contains("封面下载失败"));
    assert_eq!(result.cover_message.as_deref(), Some("封面已缓存。"));
    assert!(f
        .b
        .database()
        .unwrap()
        .get_game(&f.game)
        .unwrap()
        .description
        .unwrap()
        .starts_with("This is"));
}

#[test]
fn detail_binding_failure_retries_only_unique_enabled_source_and_reports_actual_binding() {
    let f = Fixture::new();
    save(&f.b, all_sources()).unwrap();
    let mut request = f.request();
    request.title_hint = Some("Shared Title".into());
    let calls = RefCell::new(vec![]);
    let result = confirm_with(
        &f.b,
        &request,
        |b, q| {
            calls.borrow_mut().push(q.provider.clone());
            if q.provider == "hikarinagi" {
                return Err(ServiceError(
                    ErrorCode::NetworkUnavailable,
                    "首来源请求失败。",
                ));
            }
            apply_fields(b, q, q.provider == "bangumi")
        },
        |_, q| Ok(vec![candidate(&q.providers[0], "2")]),
        true,
    )
    .unwrap();
    assert_eq!(result.provider, "bangumi");
    assert_eq!(*calls.borrow(), vec!["hikarinagi", "bangumi", "vndb"]);
    assert!(result
        .supplementation_message
        .unwrap()
        .contains("首来源请求失败"));
}

#[test]
fn hikarinagi_bilingual_intro_is_preferred_before_later_source_chinese() {
    let f = Fixture::new();
    let mut config = all_sources();
    config.sources[2].enabled = false;
    save(&f.b, config).unwrap();
    confirm_with(&f.b,&f.request(),|b,q|{let r=apply_fields(b,q,q.provider=="bangumi")?;if q.provider=="hikarinagi"{b.database()?.apply_remote_fields(&q.game_id,&q.provider,&q.remote_id,&[("description_zh".into(),"女仆 Scarlet Ikaruga Wisteria 来到宿舍，主人公与她一起生活并结识了许多朋友。".into())],&[],&now(),false)?;}Ok(r)},|_,q|Ok(vec![candidate(&q.providers[0],"2")]),true).unwrap();
    assert_eq!(
        f.b.database()
            .unwrap()
            .get_game(&f.game)
            .unwrap()
            .description
            .as_deref(),
        Some("女仆 Scarlet Ikaruga Wisteria 来到宿舍，主人公与她一起生活并结识了许多朋友。")
    );
}

#[test]
fn manual_binding_temporarily_enables_selected_source_and_preserves_global_config() {
    let f = Fixture::new();
    let mut config = all_sources();
    config.sources[0].enabled = false;
    config.sources.swap(1, 2);
    save(&f.b, config).unwrap();
    let mut request = f.request();
    assert!(confirm_with(
        &f.b,
        &request,
        |_, _| panic!("disabled"),
        |_, _| panic!("disabled"),
        true
    )
    .is_err());
    request.manual = true;
    let calls = RefCell::new(vec![]);
    confirm_with(
        &f.b,
        &request,
        |b, q| {
            calls.borrow_mut().push(q.provider.clone());
            apply_fields(b, q, q.provider == "bangumi")
        },
        |_, q| Ok(vec![candidate(&q.providers[0], "2")]),
        true,
    )
    .unwrap();
    assert_eq!(*calls.borrow(), vec!["hikarinagi", "vndb", "bangumi"]);
    assert_eq!(get(&f.b).unwrap().enabled(), vec!["vndb", "bangumi"]);
    assert!(f
        .b
        .database()
        .unwrap()
        .get_game(&f.game)
        .unwrap()
        .description
        .unwrap()
        .starts_with("This is"));
}

#[test]
fn automatic_confirmation_keeps_settings_order_even_when_requested_source_is_last() {
    let f = Fixture::new();
    save(&f.b, all_sources()).unwrap();
    let mut request = f.request();
    request.provider = "vndb".into();
    request.title_hint = Some("Shared Title".into());
    let calls = RefCell::new(vec![]);
    confirm_with(
        &f.b,
        &request,
        |b, q| {
            calls.borrow_mut().push(q.provider.clone());
            apply_fields(b, q, q.provider == "bangumi")
        },
        |_, q| Ok(vec![candidate(&q.providers[0], "2")]),
        true,
    )
    .unwrap();
    assert_eq!(*calls.borrow(), vec!["hikarinagi", "bangumi", "vndb"]);
    assert_eq!(
        f.b.database()
            .unwrap()
            .get_game(&f.game)
            .unwrap()
            .summary
            .publisher
            .as_deref(),
        Some("Foreign Publisher")
    );
}

#[test]
fn manual_search_allows_disabled_source_without_saving_settings() {
    let f = Fixture::new();
    let mut config = all_sources();
    config.sources[0].enabled = false;
    save(&f.b, config).unwrap();
    let mut request = SearchMetadataRequest {
        manual: false,
        batch_id: None,
        query: "Shared Title".into(),
        providers: vec!["hikarinagi".into()],
        cache: false,
    };
    assert!(search(&f.b, &request).is_err());
    request.manual = true;
    assert!(search(&f.b, &request).is_ok());
    assert_eq!(get(&f.b).unwrap().enabled(), vec!["bangumi", "vndb"]);
    request.providers.push("bangumi".into());
    assert!(search(&f.b, &request).is_err());
}

#[test]
fn failed_match_does_not_change_live_priority_or_metadata() {
    let f = Fixture::new();
    f.b.database()
        .unwrap()
        .set_metadata_priority(
            &f.game,
            &["bangumi".into(), "vndb".into(), "hikarinagi".into()],
        )
        .unwrap();
    let before = serde_json::to_value(f.b.database().unwrap().get_game(&f.game).unwrap()).unwrap();
    let priority: serde_json::Value =
        f.b.database()
            .unwrap()
            .setting(&format!("metadata.priority.{}", f.game))
            .unwrap()
            .unwrap();
    let mut request = f.request();
    request.manual = true;
    let result = confirm_with(
        &f.b,
        &request,
        |_, _| Err(invalid("Fixture network failure")),
        |_, _| Ok(vec![]),
        false,
    );
    assert!(result.is_err());
    assert_eq!(
        before,
        serde_json::to_value(f.b.database().unwrap().get_game(&f.game).unwrap()).unwrap()
    );
    assert_eq!(
        f.b.database()
            .unwrap()
            .setting::<serde_json::Value>(&format!("metadata.priority.{}", f.game))
            .unwrap(),
        Some(priority)
    );
}

#[test]
fn successful_match_commits_new_cache_but_failed_match_discards_it() {
    let f = Fixture::new();
    confirm_with(
        &f.b,
        &f.request(),
        |b, q| {
            b.database()?.save_metadata_cache(
                &q.provider,
                "success-cache",
                Some("{}"),
                "success",
                None,
                &now(),
            )?;
            apply_fields(b, q, true)
        },
        |_, _| Ok(vec![]),
        false,
    )
    .unwrap();
    assert!(f
        .b
        .database()
        .unwrap()
        .metadata_cache_response("hikarinagi", "success-cache")
        .unwrap()
        .is_some());
    assert!(confirm_with(
        &f.b,
        &f.request(),
        |b, q| {
            b.database()?.save_metadata_cache(
                &q.provider,
                "failed-cache",
                Some("{}"),
                "success",
                None,
                &now(),
            )?;
            Err(invalid("Fixture failure"))
        },
        |_, _| Ok(vec![]),
        false
    )
    .is_err());
    assert!(f
        .b
        .database()
        .unwrap()
        .metadata_cache_response("hikarinagi", "failed-cache")
        .unwrap()
        .is_none());
}

#[test]
fn scraper_confirmation_cannot_detach_owned_store_metadata_on_title_variations() {
    let f = Fixture::new();
    let app: crate::backend::hikarifield::App = serde_json::from_value(
        serde_json::json!({"id":7,"tag":"fixture","name":"game","have":1,"released":1}),
    )
    .unwrap();
    f.b.database()
        .unwrap()
        .import_hf_app(10, &app, None)
        .unwrap();
    let q = f.request();
    confirm_with(
        &f.b,
        &q,
        |b, q| apply_fields(b, q, true),
        |_, _| panic!("single-source confirmation"),
        false,
    )
    .unwrap();
    let detail = f.b.database().unwrap().get_game(&f.game).unwrap();
    assert_eq!(detail.summary.title, "game");
    assert_eq!(
        detail.description.as_deref(),
        Some("这是来源维护的中文简介，人物名字也保留中文。")
    );
    assert!(detail
        .metadata
        .iter()
        .any(|m| m.provider == "hikarifield" && m.field == "title"));
    assert!(detail.summary.hikari_field.is_some());
}

//! Scrape into an isolated repository first, commit only backend-owned results later.
use super::*;
use crate::{
    database::prepared_import::MetadataSnapshot,
    domain::{models::GameDetail, protocol::InstallSource, requests::ConfirmMetadataMatchRequest},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Deserialize)]
pub struct PrepareRequest {
    #[serde(default)]
    pub manual: bool,
    #[serde(default)]
    pub single_source: bool,
    #[serde(default)]
    pub title_hint: Option<String>,
    pub directory: String,
    #[serde(default)]
    pub batch_id: Option<String>,
    pub provider: String,
    pub remote_id: String,
}
#[derive(Serialize)]
pub struct Preparation {
    pub preparation_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub cover_path: String,
    pub provider: String,
    pub remote_id: String,
    pub translation_message: Option<String>,
    pub supplementation_message: Option<String>,
}
#[derive(Deserialize)]
pub struct CommitRequest {
    pub directory: String,
    pub title: String,
    pub executable_path: Option<String>,
    pub preparation_id: Option<String>,
}
#[derive(Deserialize)]
pub struct DiscardRequest {
    pub preparation_ids: Vec<String>,
}
struct PreparedItem {
    batch_id: Option<String>,
    directory: String,
    snapshot: MetadataSnapshot,
    steam_app_id: Option<String>,
}
#[derive(Default)]
pub struct PreparedImports {
    items: HashMap<String, PreparedItem>,
    batches: HashMap<String, Batch>,
    created: HashSet<String>,
    protected: HashSet<String>,
}

struct Batch {
    app_settings: app_settings::Settings,
    translation: (translation::Settings, Option<String>),
    config: metadata_sources::Config,
    cancelled: bool,
    used: HashSet<String>,
}
#[derive(Serialize)]
pub struct BeginBatch {
    pub batch_id: String,
    pub sources: metadata_sources::Config,
}
#[derive(Deserialize)]
pub struct CancelBatch {
    pub batch_id: String,
}
fn cancelled() -> ServiceError {
    ServiceError(ErrorCode::Cancelled, "本轮导入已取消。")
}
pub fn begin_batch(b: &Backend) -> Result<BeginBatch> {
    let config = metadata_sources::get(b)?;
    let app_settings = app_settings::get(b)?;
    let translation = translation::configuration(b)?;
    let batch_id = id();
    let mut m = b
        .prepared_imports
        .lock()
        .map_err(|_| invalid("导入服务需要重新启动。"))?;
    if m.batches.len() >= 10_000 {
        return Err(invalid("导入批次数量超过限制，请重新启动。"));
    }
    m.batches.insert(
        batch_id.clone(),
        Batch {
            app_settings,
            translation,
            config: config.clone(),
            cancelled: false,
            used: HashSet::new(),
        },
    );
    Ok(BeginBatch {
        batch_id,
        sources: config,
    })
}
pub(crate) fn batch_config(b: &Backend, id: &str) -> Result<metadata_sources::Config> {
    let m = b
        .prepared_imports
        .lock()
        .map_err(|_| invalid("导入服务需要重新启动。"))?;
    let batch = m.batches.get(id).ok_or_else(cancelled)?;
    if batch.cancelled {
        return Err(cancelled());
    }
    Ok(batch.config.clone())
}
/// Serialize registration and deletion: late downloads cannot recreate a cancelled batch's cache.
pub(crate) fn persist_cover(b: &Backend, path: &Path, bytes: &[u8], relative: &str) -> Result<()> {
    let mut m = b
        .prepared_imports
        .lock()
        .map_err(|_| invalid("缓存服务需要重新启动。"))?;
    if let Some(id) = &b.import_batch {
        let batch = m.batches.get_mut(id).ok_or_else(cancelled)?;
        if batch.cancelled {
            return Err(cancelled());
        }
        batch.used.insert(relative.into());
    }
    let existing = path.is_file();
    if existing && !m.created.contains(relative) || b.import_batch.is_none() {
        m.protected.insert(relative.into());
    }
    if !existing {
        let temporary = path.with_extension(format!("{}.tmp", id()));
        if std::fs::write(&temporary, bytes).is_err() {
            let _ = std::fs::remove_file(&temporary);
            return Err(permission("无法保存封面缓存。"));
        }
        if std::fs::rename(&temporary, path).is_err() {
            let _ = std::fs::remove_file(&temporary);
            return Err(permission("无法保存封面缓存。"));
        }
        if b.import_batch.is_some() {
            m.created.insert(relative.into());
        }
    }
    Ok(())
}
pub fn cancel_batch(b: &Backend, q: CancelBatch) -> Result<bool> {
    let mut m = b
        .prepared_imports
        .lock()
        .map_err(|_| invalid("导入服务需要重新启动。"))?;
    if let Some(batch) = m.batches.get_mut(&q.batch_id) {
        batch.cancelled = true;
        batch.used.clear();
    } else {
        return Err(invalid("导入批次不存在。"));
    }
    m.items
        .retain(|_, item| item.batch_id.as_deref() != Some(&q.batch_id));
    let removable = m
        .created
        .iter()
        .filter(|p| {
            !m.protected.contains(*p) && !m.batches.values().any(|batch| batch.used.contains(*p))
        })
        .cloned()
        .collect::<Vec<_>>();
    for path in removable {
        if b.database()?.cover_is_referenced(&path)? {
            m.protected.insert(path);
            continue;
        }
        let filename = path
            .strip_prefix("covers/")
            .ok_or_else(|| invalid("缓存路径无效。"))?;
        let target = b.data_directory.join("covers").join(filename);
        if target
            .ancestors()
            .any(|p| std::fs::symlink_metadata(p).is_ok_and(|m| scanner::linked(&m)))
        {
            return Err(permission("缓存路径包含链接，导入已取消但未执行缓存删除。"));
        }
        match std::fs::remove_file(target) {
            Ok(()) => {
                m.created.remove(&path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                m.created.remove(&path);
            }
            Err(_) => {
                return Err(permission(
                    "导入已取消，但部分缓存无法清理；请再次取消重试。",
                ))
            }
        }
    }
    Ok(true)
}
pub(crate) fn validate_cover(backend: &Backend, path: &str) -> Result<()> {
    let filename = path
        .strip_prefix("covers/")
        .ok_or_else(|| invalid("封面尚未保存到本地，不计入刮削成功。"))?;
    let response = crate::cover_protocol::response(
        Some(&backend.data_directory),
        &format!("/{filename}"),
        "http://tauri.localhost",
    );
    let bytes = response.body();
    let hash = format!("{:x}", Sha256::digest(bytes));
    if response.status() != tauri::http::StatusCode::OK
        || filename.split('.').next() != Some(hash.as_str())
    {
        return Err(ServiceError(
            ErrorCode::InvalidResponse,
            "本地封面缺失或损坏，请重新刮削该条目。",
        ));
    }
    super::thumbnails::bounded_image(bytes).map_err(|_| {
        ServiceError(
            ErrorCode::InvalidResponse,
            "本地封面损坏或无法显示，请重新刮削该条目。",
        )
    })?;
    Ok(())
}

pub fn prepare(backend: &Backend, request: PrepareRequest) -> Result<Preparation> {
    prepare_with(backend, request, |stage, request| {
        if request.provider == "steam" {
            super::steam::confirm(stage, request, None, true)
        } else {
            metadata_sources::confirm(stage, request).map(|result| result.translation_message)
        }
    })
}
pub(super) fn prepare_with(
    backend: &Backend,
    request: PrepareRequest,
    fetch: impl FnOnce(&Backend, &ConfirmMetadataMatchRequest) -> Result<Option<String>>,
) -> Result<Preparation> {
    if !matches!(
        request.provider.as_str(),
        "bangumi" | "vndb" | "hikarinagi" | "steam"
    ) {
        return Err(invalid("请选择支持的资料源。"));
    }
    let directory = absolute_directory(&request.directory)?;
    let directory_text = path_text(&directory)?;
    if request.provider == "steam" {
        super::steam::scan::verify(&directory_text, &request.remote_id)?;
        if backend
            .database()?
            .steam_import_conflict(&request.remote_id, &directory_text)?
            .is_some()
        {
            return Err(ServiceError(
                ErrorCode::Conflict,
                "该 Steam 游戏已在库中，请刷新列表。",
            ));
        }
    }
    if backend
        .database()?
        .installation_conflict(&directory_text)?
        .is_some()
    {
        return Err(ServiceError(
            ErrorCode::Conflict,
            "该路径已在游戏库中，请排除重复项。",
        ));
    }
    let mut stage = backend.clone();
    if let Some(batch_id) = request.batch_id.as_deref() {
        stage.metadata_snapshot = Some(batch_config(backend, batch_id)?);
        stage.import_batch = Some(batch_id.into());
    }
    if request.single_source && request.provider != "steam" {
        let mut config = metadata_sources::get(&stage)?;
        for source in &mut config.sources {
            source.enabled = source.provider == request.provider;
        }
        stage.metadata_snapshot = Some(config);
    }
    if let Some(batch_id) = &request.batch_id {
        let manager = backend
            .prepared_imports
            .lock()
            .map_err(|_| invalid("导入服务不可用。"))?;
        let batch = manager.batches.get(batch_id).ok_or_else(cancelled)?;
        stage.app_settings_snapshot = Some(batch.app_settings.clone());
        stage.translation_snapshot = Some(batch.translation.clone());
    } else {
        stage.app_settings_snapshot = Some(app_settings::get(backend)?);
        stage.translation_snapshot = Some(translation::configuration(backend)?);
    }
    stage.db = Arc::new(Mutex::new(Database::in_memory()?));
    // Same credential-vault scope and source implementations, isolated game/cache rows.
    for name in [
        "metadata.translation",
        super::app_settings::KEY,
        super::hikarinagi_settings::SETTING,
        super::metadata_sources::SETTING,
    ] {
        if let Some(settings) = backend.database()?.setting::<serde_json::Value>(name)? {
            stage.database()?.put_setting(name, &settings)?;
        }
    }
    if let Some((settings, _)) = &stage.translation_snapshot {
        stage.database()?.save_translation_config(settings)?;
    }
    let folder = directory.file_name().unwrap_or_default().to_string_lossy();
    let game_id = stage.database()?.import_installation(
        &directory_text,
        &folder,
        None,
        InstallSource::Manual,
        &[],
        "",
    )?;
    let translation_message = fetch(
        &stage,
        &ConfirmMetadataMatchRequest {
            title_hint: request.title_hint.clone(),
            game_id: game_id.clone(),
            provider: request.provider.clone(),
            remote_id: request.remote_id.clone(),
            manual: request.manual,
        },
    )?;
    let snapshot = stage.database()?.prepared_metadata(&game_id)?;
    validate_cover(backend, &snapshot.cover_path)?;
    let binding = snapshot
        .priority
        .iter()
        .find_map(|provider| {
            snapshot.fields.iter().find(|field| {
                &field.provider == provider
                    && matches!(
                        field.field.as_str(),
                        "title" | "title_zh" | "title_en" | "title_ja" | "title_alt"
                    )
            })
        })
        .ok_or_else(|| invalid("完整资料尚未准备完成，请重试。"))?;
    let preparation_id = id();
    let response = Preparation {
        preparation_id: preparation_id.clone(),
        title: snapshot.title.clone(),
        subtitle: snapshot.title_ja.clone(),
        cover_path: snapshot.cover_path.clone(),
        provider: binding.provider.clone(),
        remote_id: binding.remote_id.clone(),
        translation_message: if request.provider == "steam" {
            None
        } else {
            translation_message.clone()
        },
        supplementation_message: if request.provider == "steam" {
            translation_message
        } else {
            None
        },
    };
    let mut manager = backend
        .prepared_imports
        .lock()
        .map_err(|_| invalid("刮削服务需要重新启动。"))?;
    if manager.items.len() >= 10_000 {
        return Err(invalid("暂存条目过多，请完成或取消已有导入后再试。"));
    }
    if let Some(batch_id) = &request.batch_id {
        if manager
            .batches
            .get(batch_id)
            .is_none_or(|batch| batch.cancelled)
        {
            return Err(cancelled());
        }
    }
    manager.items.insert(
        preparation_id,
        PreparedItem {
            batch_id: request.batch_id,
            directory: directory_text,
            snapshot,
            steam_app_id: (request.provider == "steam").then_some(request.remote_id),
        },
    );
    Ok(response)
}

pub fn discard(backend: &Backend, request: DiscardRequest) -> Result<bool> {
    if request.preparation_ids.len() > 10_000 {
        return Err(invalid("暂存条目数量无效。"));
    }
    let mut manager = backend
        .prepared_imports
        .lock()
        .map_err(|_| invalid("刮削服务需要重新启动。"))?;
    for id in request.preparation_ids {
        manager.items.remove(&id);
    }
    Ok(true)
}

/// No source requests, translations or image downloads are reachable from this path.
pub fn commit(backend: &Backend, request: CommitRequest) -> Result<GameDetail> {
    commit_with(backend, request, None)
}
pub(super) fn commit_steam(
    backend: &Backend,
    request: CommitRequest,
    app_id: &str,
) -> Result<GameDetail> {
    super::steam::scan::verify(&request.directory, app_id)?;
    commit_with(backend, request, Some(app_id))
}
fn commit_with(
    backend: &Backend,
    request: CommitRequest,
    steam_app_id: Option<&str>,
) -> Result<GameDetail> {
    let directory = absolute_directory(&request.directory)?;
    let directory_text = path_text(&directory)?;
    let candidates = if steam_app_id.is_some() {
        vec![]
    } else {
        scanner::directory_candidates(&directory)?
    };
    let executable = request
        .executable_path
        .as_deref()
        .map(|value| {
            let path = Path::new(value)
                .canonicalize()
                .map_err(|_| invalid("启动 EXE 已不可访问，请重新选择。"))?;
            if !path.starts_with(&directory)
                || !path.is_file()
                || !candidates
                    .iter()
                    .any(|candidate| Path::new(&candidate.path) == path)
            {
                return Err(invalid("启动 EXE 必须是当前游戏目录内的可用候选。"));
            }
            path_text(&path)
        })
        .transpose()?;
    let mut manager = backend
        .prepared_imports
        .lock()
        .map_err(|_| invalid("刮削服务需要重新启动。"))?;
    let prepared = request
        .preparation_id
        .as_ref()
        .map(|id| {
            let item = manager
                .items
                .get(id)
                .ok_or_else(|| invalid("暂存资料已失效，请重新刮削该条目。"))?;
            if item.steam_app_id.as_deref() != steam_app_id {
                return Err(invalid("暂存资料与 Steam 游戏身份不一致。"));
            }
            if item.directory != directory_text {
                return Err(invalid("暂存资料与导入目录不一致。"));
            }
            validate_cover(backend, &item.snapshot.cover_path)?;
            Ok(&item.snapshot)
        })
        .transpose()?;
    let game = if let Some(app_id) = steam_app_id {
        backend.database()?.import_prepared_steam(
            &directory_text,
            &request.title,
            app_id,
            prepared,
        )?
    } else {
        backend.database()?.import_prepared(
            &directory_text,
            &request.title,
            executable.as_deref(),
            &candidates,
            prepared,
        )?
    };
    if let Some(id) = request.preparation_id {
        manager.items.remove(&id);
    }
    Ok(game)
}

#[cfg(test)]
mod tests;

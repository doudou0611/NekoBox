use super::{types::*, *};
use crate::domain::{
    models::{MetadataCandidate, ScanRootsRequest, ScanTask, TaskProgress},
    protocol::{InstallSource, TaskStatus},
    EventEnvelope,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
    time::{Duration, Instant},
};
use tauri::Emitter;

const MAX_ENTRIES: u64 = 100_000;
const MAX_DEPTH: usize = 32;
const MAX_IMPORT_DEPTH: usize = 7;
const MAX_CANDIDATES: u64 = 10_000;
#[derive(Default)]
pub struct ScanManager {
    controls: Mutex<HashMap<String, Arc<AtomicU8>>>,
}
impl ScanManager {
    pub(crate) fn is_idle(&self) -> Result<bool> {
        Ok(self
            .controls
            .lock()
            .map_err(|_| invalid("扫描服务不可用。"))?
            .is_empty())
    }
}
pub fn ignored_executable(path: &Path) -> bool {
    let name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    [
        "unins",
        "uninstall",
        "setup",
        "installer",
        "update",
        "updater",
        "crash",
        "crashpad",
        "vcredist",
        "vc_redist",
        "dotnet",
        "redistributable",
        "launcher_helper",
        "crashreporter",
        "dxsetup",
        "dxwebsetup",
        "directx",
        "config",
        "settings",
        "patch",
        "galgame-manager",
        "nekobox",
        "删除",
        "卸载",
    ]
    .iter()
    .any(|s| name.contains(s))
}
fn ignored_directory(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    name.starts_with('.')
        || [
            "node_modules",
            "cache",
            "caches",
            "__pycache__",
            "redist",
            "redistributables",
            "patch",
            "patches",
            "$recycle.bin",
            "system volume information",
            ".tools",
        ]
        .contains(&name.as_str())
}
pub(crate) fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn issue(report: &mut ScanReport, path: &Path, reason: &str) {
    report.issue_count += 1;
    if report.issues.len() < 100 {
        report.issues.push(ScanIssue {
            path: path.to_string_lossy().into_owned(),
            reason: reason.into(),
        });
    }
}
/// Read launchers within one game boundary, descending only if a directory
/// has no usable EXE. Never execute files or split nested folders into games.
pub fn directory_candidates(directory: &Path) -> Result<Vec<ExecutableCandidate>> {
    let mut entries_seen = 0;
    let mut report = ImportPreviewReport {
        items: Vec::new(),
        scanned_directories: 0,
        skipped_directories: 0,
        issue_count: 0,
        issues: Vec::new(),
    };
    collect_preview_executables(
        directory,
        0,
        MAX_IMPORT_DEPTH,
        &mut report,
        &mut entries_seen,
    )
}

fn is_launchable(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

fn inspect_launchable(path: &Path) -> Result<ExecutableCandidate> {
    pe::inspect(path)
}

fn selected_executable(directory: &Path, candidates: &[ExecutableCandidate]) -> Option<String> {
    let first = candidates.first()?;
    if candidates.len() == 1 {
        return Some(first.path.clone());
    }
    let folder = directory
        .file_name()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if let Some(candidate) = candidates.iter().find(|candidate| {
        let stem = Path::new(&candidate.path)
            .file_stem()
            .map(|value| value.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        !stem.is_empty() && (stem.contains(&folder) || folder.contains(&stem))
    }) {
        return Some(candidate.path.clone());
    }
    candidates
        .iter()
        .max_by_key(|candidate| fs::metadata(&candidate.path).map(|m| m.len()).unwrap_or(0))
        .map(|candidate| candidate.path.clone())
        .or_else(|| Some(first.path.clone()))
}

fn preview_search_name(directory: &Path, _candidates: &[ExecutableCandidate]) -> String {
    directory
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "未命名游戏".into())
}

fn preview_issue(report: &mut ImportPreviewReport, path: &Path, reason: &str) {
    report.issue_count += 1;
    if report.issues.len() < 100 {
        report.issues.push(ScanIssue {
            path: path.to_string_lossy().into_owned(),
            reason: reason.into(),
        });
    }
}

fn collect_preview_executables(
    directory: &Path,
    depth: usize,
    max_depth: usize,
    report: &mut ImportPreviewReport,
    entries_seen: &mut u64,
) -> Result<Vec<ExecutableCandidate>> {
    report.scanned_directories += 1;
    let entries = fs::read_dir(directory)
        .map_err(|_| ServiceError(ErrorCode::PermissionDenied, "目录无法读取。"))?;
    let mut launchers = Vec::new();
    let mut children = Vec::new();
    for entry in entries {
        *entries_seen += 1;
        if *entries_seen > MAX_ENTRIES {
            return Err(invalid("目录条目超过导入预览限制，请缩小选择范围。"));
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                preview_issue(report, directory, "无法读取目录条目。");
                continue;
            }
        };
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                preview_issue(report, &path, "无法读取文件信息。");
                continue;
            }
        };
        if linked(&metadata) {
            report.skipped_directories += 1;
            preview_issue(report, &path, "跳过符号链接或重解析点。");
            continue;
        }
        if metadata.is_dir() {
            if ignored_directory(&path) {
                report.skipped_directories += 1;
            } else {
                children.push(path);
            }
        } else if metadata.is_file() && is_launchable(&path) && !ignored_executable(&path) {
            match inspect_launchable(&path) {
                Ok(candidate) => launchers.push(candidate),
                Err(error) => preview_issue(report, &path, error.1),
            }
        }
    }
    if !launchers.is_empty() {
        launchers.sort_by(|left, right| left.path.cmp(&right.path));
        return Ok(launchers);
    }
    if depth >= max_depth {
        if max_depth > 0 && !children.is_empty() {
            preview_issue(report, directory, "达到导入预览最大深度，跳过更深目录。");
        }
        return Ok(launchers);
    }
    children.sort();
    for child in children {
        match collect_preview_executables(&child, depth + 1, max_depth, report, entries_seen) {
            Ok(found) => launchers.extend(found),
            Err(error) if *entries_seen > MAX_ENTRIES => return Err(error),
            Err(error) => preview_issue(report, &child, error.1),
        }
    }
    launchers.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(launchers)
}

fn preview_walk(
    directory: &Path,
    max_depth: usize,
    report: &mut ImportPreviewReport,
    items: &mut Vec<ImportPreviewCandidate>,
    entries_seen: &mut u64,
) -> Result<()> {
    let launchers = collect_preview_executables(directory, 0, max_depth, report, entries_seen)?;
    if !launchers.is_empty() || has_directory_content(directory) {
        let directory = directory
            .canonicalize()
            .unwrap_or_else(|_| directory.into());
        let folder_name = directory
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        items.push(ImportPreviewCandidate {
            directory: path_text(&directory)?,
            folder_name,
            search_name: preview_search_name(&directory, &launchers),
            selected_executable: selected_executable(&directory, &launchers),
            executables: launchers,
            existing_game_id: None,
            duplicate_reason: None,
        });
        if items.len() as u64 > MAX_CANDIDATES {
            return Err(invalid("候选数量超过导入预览限制，请缩小选择范围。"));
        }
    }
    Ok(())
}

pub fn preview_import(
    backend: &Backend,
    request: &PreviewImportRequest,
) -> Result<ImportPreviewReport> {
    if request.follow_symlinks || request.roots.is_empty() || request.roots.len() > 16 {
        return Err(invalid("需要 1～16 个根目录，且不能跟随符号链接。"));
    }
    let mut items = Vec::new();
    let mut entries_seen = 0;
    let mut report = ImportPreviewReport {
        items: Vec::new(),
        scanned_directories: 0,
        skipped_directories: 0,
        issue_count: 0,
        issues: Vec::new(),
    };
    if request.single_directory {
        if request.roots.len() != 1 || request.single_executable.is_some() {
            return Err(invalid("单个目录导入只接受一个游戏文件夹。"));
        }
        let root = absolute_directory(&request.roots[0])?;
        preview_walk(
            &root,
            MAX_IMPORT_DEPTH,
            &mut report,
            &mut items,
            &mut entries_seen,
        )?;
    } else if let Some(executable) = &request.single_executable {
        let path = Path::new(executable);
        if !path.is_absolute()
            || !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            return Err(ServiceError(
                ErrorCode::PathInvalid,
                "单个导入必须选择绝对路径的 EXE 文件。",
            ));
        }
        let executable = path
            .canonicalize()
            .map_err(|_| ServiceError(ErrorCode::PathInvalid, "EXE 不存在或无法访问。"))?;
        if !executable.is_file() || ignored_executable(&executable) {
            return Err(ServiceError(
                ErrorCode::PathInvalid,
                "所选 EXE 不是可导入的游戏入口。",
            ));
        }
        let directory = executable
            .parent()
            .ok_or(ServiceError(ErrorCode::PathInvalid, "EXE 所在目录无效。"))?;
        preview_walk(directory, 0, &mut report, &mut items, &mut entries_seen)?;
        let directory = path_text(directory)?;
        let selected = path_text(&executable)?;
        let item = items
            .iter_mut()
            .find(|item| item.directory == directory)
            .ok_or(ServiceError(
                ErrorCode::PathInvalid,
                "所选目录没有可用 EXE。",
            ))?;
        if !item
            .executables
            .iter()
            .any(|candidate| candidate.path == selected)
        {
            return Err(ServiceError(
                ErrorCode::PathInvalid,
                "所选 EXE 未通过只读识别。",
            ));
        }
        item.selected_executable = Some(selected);
    } else {
        for value in &request.roots {
            let root = absolute_directory(value)?;
            let boundaries = game_boundaries(&root)?;
            for boundary in boundaries {
                match preview_walk(
                    &boundary,
                    MAX_IMPORT_DEPTH,
                    &mut report,
                    &mut items,
                    &mut entries_seen,
                ) {
                    Ok(()) => {}
                    Err(error)
                        if entries_seen > MAX_ENTRIES || items.len() as u64 > MAX_CANDIDATES =>
                    {
                        return Err(error)
                    }
                    Err(error) => preview_issue(&mut report, &boundary, error.1),
                }
            }
        }
    }
    let db = backend.database()?;
    for item in &mut items {
        if let Some(game_id) = db.installation_conflict(&item.directory)? {
            item.existing_game_id = Some(game_id);
            item.duplicate_reason = Some("路径已在游戏库中，确认后将跳过重复导入。".into());
        }
    }
    report.items = items;
    Ok(report)
}

/// Folder-first discovery: when a root contains child folders, each immediate child is one game
/// boundary. We recurse within that boundary but never turn nested folders into extra games.
fn game_boundaries(root: &Path) -> Result<Vec<PathBuf>> {
    let mut children = vec![];
    for entry in fs::read_dir(root)
        .map_err(|_| ServiceError(ErrorCode::PermissionDenied, "目录无法读取。"))?
    {
        let entry =
            entry.map_err(|_| ServiceError(ErrorCode::PermissionDenied, "目录条目无法读取。"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| ServiceError(ErrorCode::PermissionDenied, "文件无法读取。"))?;
        if metadata.is_dir() && !linked(&metadata) && !ignored_directory(&path) {
            children
                .push(path.canonicalize().map_err(|_| {
                    ServiceError(ErrorCode::PermissionDenied, "游戏目录无法访问。")
                })?);
        }
    }
    children.sort();
    if children.is_empty() {
        Ok(vec![root.to_path_buf()])
    } else {
        Ok(children)
    }
}
fn fingerprint(candidates: &[ExecutableCandidate]) -> String {
    let mut hash = Sha256::new();
    for c in candidates {
        hash.update(c.path.as_bytes());
        hash.update(c.fingerprint.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
fn has_directory_content(directory: &Path) -> bool {
    fs::read_dir(directory).ok().is_some_and(|entries| {
        entries
            .filter_map(std::result::Result::ok)
            .any(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
    })
}
pub fn manual_import(
    backend: &Backend,
    r: &ImportGameRequest,
) -> Result<crate::domain::models::GameDetail> {
    let directory = absolute_directory(&r.directory)?;
    let candidates = directory_candidates(&directory)?;
    let id = backend.database()?.import_installation(
        &path_text(&directory)?,
        &r.title,
        r.game_id.as_deref(),
        InstallSource::Manual,
        &candidates,
        &fingerprint(&candidates),
    )?;
    #[cfg(not(test))]
    {
        let should_match = backend
            .database()
            .and_then(|db| db.get_game(&id))
            .is_ok_and(|game| {
                matches!(
                    game.summary.metadata_status,
                    crate::domain::protocol::MetadataStatus::LocalOnly
                )
            });
        if should_match && !r.skip_metadata {
            let _ = automatic_metadata_match(backend, &id, &r.title);
        }
    }
    backend.database()?.get_game(&id)
}

/// Follow the configured sources, retaining ambiguous identities for manual review.
fn automatic_metadata_match(backend: &Backend, game_id: &str, query: &str) -> Result<()> {
    if backend.database()?.metadata_locked(game_id)? {
        return Ok(());
    }
    let query = vndb::automatic_query(query);
    let mut candidates = Vec::new();
    let mut snapshot = backend.clone();
    snapshot.metadata_snapshot = Some(super::metadata_sources::get(backend)?);
    let backend = &snapshot;
    for provider in super::metadata_sources::get(backend)?.enabled() {
        let request = crate::domain::requests::SearchMetadataRequest {
            manual: false,
            batch_id: None,
            query: query.clone(),
            providers: vec![provider.clone()],
            cache: true,
        };
        let mut found = match provider.as_str() {
            "bangumi" => bangumi::search_metadata(backend, &request),
            "hikarinagi" => super::hikarinagi::search(backend, &request),
            _ => vndb::search(backend, &request),
        }
        .unwrap_or_default();
        found.sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
        if let Some(best) = automatic_candidate(&found) {
            let request = crate::domain::requests::ConfirmMetadataMatchRequest {
                manual: false,
                title_hint: Some(best.title.clone()),
                game_id: game_id.into(),
                provider: provider.clone(),
                remote_id: best.remote_id.clone(),
            };
            // All fields and supplements follow the same source snapshot.
            if confirm_automatic_candidate(backend, &request).is_ok() {
                return Ok(());
            }
        }
        candidates.extend(found.into_iter().take(5));
    }
    candidates.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.provider.cmp(&right.provider))
            .then_with(|| left.remote_id.cmp(&right.remote_id))
    });
    candidates.dedup_by(|left, right| {
        left.provider == right.provider && left.remote_id == right.remote_id
    });
    if backend.database()?.metadata_locked(game_id)? {
        return Ok(());
    }
    if candidates.is_empty() {
        backend
            .database()?
            .delete_setting(&format!("metadata.candidates.{game_id}"))?;
    } else {
        backend
            .database()?
            .put_setting(&format!("metadata.candidates.{game_id}"), &candidates)?;
    }
    Ok(())
}

fn confirm_automatic_candidate(
    backend: &Backend,
    request: &crate::domain::requests::ConfirmMetadataMatchRequest,
) -> Result<()> {
    super::metadata_sources::confirm(backend, request)?;
    backend
        .database()?
        .delete_setting(&format!("metadata.candidates.{}", request.game_id))?;
    Ok(())
}

fn automatic_candidate(candidates: &[MetadataCandidate]) -> Option<&MetadataCandidate> {
    let best = candidates.first()?;
    let unique = candidates
        .get(1)
        .is_none_or(|next| best.confidence - next.confidence >= 0.05);
    (best.confidence >= 0.95 && unique).then_some(best)
}

impl Backend {
    pub fn scan_report(&self, id: &str) -> Result<ScanReport> {
        self.database()?
            .setting(&format!("scan.task.{id}"))?
            .ok_or_else(missing)
    }
    pub fn control_scan(&self, r: &ControlScanRequest) -> Result<ScanReport> {
        let controls = self
            .scans
            .controls
            .lock()
            .map_err(|_| ServiceError(ErrorCode::InternalError, "扫描服务不可用。"))?;
        let flag = controls.get(&r.task_id).ok_or(ServiceError(
            ErrorCode::Conflict,
            "扫描已经结束，不能再暂停或取消。",
        ))?;
        if flag.load(Ordering::SeqCst) == 2 {
            return Err(ServiceError(ErrorCode::Conflict, "扫描已经请求取消。"));
        }
        flag.store(
            match r.action {
                ScanAction::Pause => 1,
                ScanAction::Resume => 0,
                ScanAction::Cancel => 2,
            },
            Ordering::SeqCst,
        );
        drop(controls);
        self.scan_report(&r.task_id)
    }
    pub fn start_scan(
        &self,
        r: &ScanRootsRequest,
        request_id: &str,
        app: Option<tauri::AppHandle>,
    ) -> Result<ScanTask> {
        if r.follow_symlinks || r.roots.is_empty() || r.roots.len() > 16 {
            return Err(invalid("需要 1～16 个根目录，且不能跟随符号链接。"));
        }
        let mut roots = vec![];
        for value in &r.roots {
            if Path::new(value)
                .ancestors()
                .any(|p| fs::symlink_metadata(p).is_ok_and(|m| linked(&m)))
            {
                return Err(ServiceError(
                    ErrorCode::PathInvalid,
                    "扫描根目录不能经过符号链接或重解析点。",
                ));
            }
            let directory = absolute_directory(value)?;
            if !roots.contains(&directory) {
                roots.push(directory);
            }
        }
        let mut controls = self
            .scans
            .controls
            .lock()
            .map_err(|_| ServiceError(ErrorCode::InternalError, "扫描服务不可用。"))?;
        if !controls.is_empty() {
            return Err(ServiceError(
                ErrorCode::Conflict,
                "已有扫描正在进行，请等待或取消后重试。",
            ));
        }
        let task_id = id();
        let flag = Arc::new(AtomicU8::new(0));
        let report = ScanReport {
            progress: TaskProgress {
                task_id: task_id.clone(),
                status: TaskStatus::Queued,
                phase: "queued".into(),
                processed: 0,
                total: None,
                message: "等待扫描。".into(),
            },
            imported: 0,
            unchanged: 0,
            issue_count: 0,
            issues: vec![],
            truncated: false,
        };
        self.persist_report(&report, request_id, app.as_ref())?;
        self.database()?.put_setting("scan.last_task", &task_id)?;
        controls.insert(task_id.clone(), flag.clone());
        drop(controls);
        let mut backend = self.clone();
        backend.metadata_snapshot = Some(super::metadata_sources::get(self)?);
        let request_id = request_id.to_owned();
        let task = task_id.clone();
        std::thread::spawn(move || {
            let mut report = report;
            let outcome = scan(
                &backend,
                roots,
                &flag,
                &mut report,
                &request_id,
                app.as_ref(),
            );
            if let Err(e) = outcome {
                report.progress.status = TaskStatus::Failed;
                report.progress.phase = "failed".into();
                report.progress.message = e.1.into();
            }
            if backend
                .persist_report(&report, &request_id, app.as_ref())
                .is_err()
            {
                eprintln!("扫描状态写入失败，请检查便携数据目录。");
            }
            if let Ok(mut controls) = backend.scans.controls.lock() {
                controls.remove(&task);
            }
        });
        Ok(ScanTask {
            task_id,
            status: TaskStatus::Queued,
        })
    }
    fn persist_report(
        &self,
        r: &ScanReport,
        request: &str,
        app: Option<&tauri::AppHandle>,
    ) -> Result<()> {
        self.database()?
            .put_setting(&format!("scan.task.{}", r.progress.task_id), r)?;
        if let Some(app) = app {
            let _ = app.emit(
                "scan:progress",
                EventEnvelope {
                    request_id: request.to_owned(),
                    occurred_at: now(),
                    payload: r.progress.clone(),
                },
            );
        }
        Ok(())
    }
}
fn checkpoint(
    backend: &Backend,
    flag: &AtomicU8,
    report: &mut ScanReport,
    request: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<bool> {
    while flag.load(Ordering::SeqCst) == 1 {
        if report.progress.status != TaskStatus::Paused {
            report.progress.status = TaskStatus::Paused;
            report.progress.phase = "paused".into();
            report.progress.message = "扫描已暂停。".into();
            backend.persist_report(report, request, app)?;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if flag.load(Ordering::SeqCst) == 2 {
        report.progress.status = TaskStatus::Cancelled;
        report.progress.phase = "cancelled".into();
        report.progress.message = "扫描已取消，已导入记录保留。".into();
        return Ok(false);
    }
    report.progress.status = TaskStatus::Running;
    report.progress.phase = "walking".into();
    Ok(true)
}
fn scan(
    backend: &Backend,
    roots: Vec<PathBuf>,
    flag: &AtomicU8,
    report: &mut ScanReport,
    request: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<()> {
    let mut boundaries = vec![];
    for root in roots {
        boundaries.extend(game_boundaries(&root)?);
    }
    boundaries.sort();
    boundaries.dedup();
    let mut seen = HashSet::new();
    let mut candidate_count = 0u64;
    let mut last = Instant::now();
    for boundary in boundaries {
        if !checkpoint(backend, flag, report, request, app)? {
            return Ok(());
        }
        let boundary = match boundary.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                issue(report, &boundary, "目录不存在或无法访问。");
                continue;
            }
        };
        if boundary == backend.data_directory || !seen.insert(boundary.clone()) {
            continue;
        }
        let mut stack = vec![(boundary.clone(), 0usize)];
        let mut candidates = vec![];
        while let Some((directory, depth)) = stack.pop() {
            if !checkpoint(backend, flag, report, request, app)? {
                return Ok(());
            }
            if directory
                .ancestors()
                .any(|p| fs::symlink_metadata(p).is_ok_and(|m| linked(&m)))
            {
                issue(report, &directory, "跳过符号链接或重解析点。");
                continue;
            }
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(_) => {
                    issue(report, &directory, "拒绝访问目录。");
                    continue;
                }
            };
            let mut child_directories = Vec::new();
            let mut found_launchable = false;
            for entry in entries {
                if !checkpoint(backend, flag, report, request, app)? {
                    return Ok(());
                }
                if report.progress.processed >= MAX_ENTRIES {
                    report.truncated = true;
                    break;
                }
                report.progress.processed += 1;
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => {
                        issue(report, &directory, "无法读取目录条目。");
                        continue;
                    }
                };
                let path = entry.path();
                let metadata = match fs::symlink_metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(_) => {
                        issue(report, &path, "无法读取文件信息。");
                        continue;
                    }
                };
                if linked(&metadata) {
                    continue;
                }
                if metadata.is_dir() && !ignored_directory(&path) {
                    child_directories.push(path);
                } else if metadata.is_file() && is_launchable(&path) && !ignored_executable(&path) {
                    found_launchable = true;
                    match inspect_launchable(&path) {
                        Ok(candidate) => {
                            candidate_count += 1;
                            if candidate_count > MAX_CANDIDATES {
                                report.truncated = true;
                                break;
                            }
                            candidates.push(candidate);
                        }
                        Err(error) => issue(report, &path, error.1),
                    }
                }
            }
            if !found_launchable {
                for path in child_directories {
                    if depth < MAX_DEPTH {
                        stack.push((path, depth + 1));
                    } else {
                        report.truncated = true;
                        issue(report, &path, "达到最大扫描深度。");
                    }
                }
            }
            if report.progress.processed >= MAX_ENTRIES || candidate_count > MAX_CANDIDATES {
                break;
            }
        }
        // A selected game boundary is useful even when no PE launcher was found:
        // metadata can still be scraped and the user can configure an entry later.
        if !candidates.is_empty() || has_directory_content(&boundary) {
            candidates.sort_by(|a, b| a.path.cmp(&b.path));
            let key = fingerprint(&candidates);
            let directory_text = path_text(&boundary)?;
            if !checkpoint(backend, flag, report, request, app)? {
                return Ok(());
            }
            let mut db = backend.database()?;
            let game_id = if db.scan_fingerprint(&directory_text)?.as_ref() == Some(&key) {
                report.unchanged += 1;
                db.game_id_for_installation(&directory_text)?
            } else {
                let title = preview_search_name(&boundary, &candidates);
                match db.import_installation(
                    &directory_text,
                    &title,
                    None,
                    InstallSource::Local,
                    &candidates,
                    &key,
                ) {
                    Ok(game_id) => {
                        report.imported += 1;
                        Some(game_id)
                    }
                    Err(error) => {
                        issue(report, &boundary, error.1);
                        None
                    }
                }
            };
            drop(db);
            if app.is_some() {
                if let Some(game_id) = game_id {
                    let title = preview_search_name(&boundary, &candidates);
                    let should_match = backend
                        .database()
                        .and_then(|db| db.get_game(&game_id))
                        .is_ok_and(|game| {
                            matches!(
                                game.summary.metadata_status,
                                crate::domain::protocol::MetadataStatus::LocalOnly
                            )
                        });
                    if should_match {
                        report.progress.message = format!("正在自动匹配资料：{title}");
                        backend.persist_report(report, request, app)?;
                        if automatic_metadata_match(backend, &game_id, &title).is_err() {
                            issue(report, &boundary, "资料自动匹配失败，已保留本地记录。")
                        }
                    }
                }
            }
        }
        if last.elapsed() >= Duration::from_millis(300) {
            report.progress.message = format!(
                "已检查 {} 个条目，导入 {}，未变更 {}。",
                report.progress.processed, report.imported, report.unchanged
            );
            backend.persist_report(report, request, app)?;
            last = Instant::now();
        }
        if report.progress.processed >= MAX_ENTRIES || candidate_count > MAX_CANDIDATES {
            break;
        }
    }
    if !checkpoint(backend, flag, report, request, app)? {
        return Ok(());
    }
    report.progress.status = TaskStatus::Completed;
    report.progress.phase = if report.truncated {
        "limited"
    } else {
        "completed"
    }
    .into();
    report.progress.message = format!(
        "扫描结束：导入 {}，未变更 {}，问题 {}{}。",
        report.imported,
        report.unchanged,
        report.issue_count,
        if report.truncated {
            "，部分目录因限制未扫描"
        } else {
            ""
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod brand_tests {
    use super::*;
    #[test]
    fn own_application_is_not_a_game_candidate() {
        assert!(ignored_executable(Path::new("NekoBox.exe")));
        assert!(ignored_executable(Path::new("Galgame-Manager.exe")));
        assert!(!ignored_executable(Path::new("game.exe")));
    }
}

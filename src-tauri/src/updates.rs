//! GitHub Releases is the source of truth. Only signed NSIS assets can be installed.
use crate::{
    backend::{self, types::EmptyRequest, Backend, BackendState, ServiceError},
    domain::{protocol::valid_request_id, ApiRequest, ApiResponse, ErrorCode},
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{io::Read, sync::Mutex, time::Duration};
use tauri::Manager;
use tauri_plugin_updater::{Update, UpdaterExt};

const REPOSITORY: &str = "https://github.com/doudou0611/NekoBox";
const RELEASE_API: &str = "https://api.github.com/repos/doudou0611/NekoBox/releases/latest";

#[derive(Clone, Serialize)]
pub struct UpdateStatus {
    pub phase: String,
    pub current_version: String,
    pub architecture: String,
    pub installation: String,
    pub version: Option<String>,
    pub notes: String,
    pub release_url: String,
    pub download_url: Option<String>,
    pub signed: bool,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub message: String,
}
impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            phase: "idle".into(),
            current_version: env!("CARGO_PKG_VERSION").into(),
            architecture: architecture().into(),
            installation: installation().into(),
            version: None,
            notes: String::new(),
            release_url: format!("{REPOSITORY}/releases/latest"),
            download_url: None,
            signed: false,
            downloaded: 0,
            total: None,
            message: "启动时自动检查，也可以手动检查更新。".into(),
        }
    }
}
#[derive(Default)]
pub struct UpdateState(Mutex<Pending>);
#[derive(Default)]
struct Pending {
    status: UpdateStatus,
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
}
#[derive(Deserialize)]
pub struct InstallRequest {
    pub confirmed: bool,
}
#[derive(Deserialize)]
pub struct OpenRequest {
    pub download: bool,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}
fn architecture() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => "unsupported",
    }
}
fn installation() -> &'static str {
    if !cfg!(windows) {
        "unsupported"
    } else if matches!(
        tauri::utils::platform::bundle_type(),
        Some(tauri::utils::config::BundleType::Nsis)
    ) {
        "installer"
    } else {
        "portable"
    }
}
fn release_version(tag: &str) -> backend::Result<Version> {
    let tag = tag.trim_start_matches(['v', 'V']);
    // The first published release used V0.1. Future tags must use full SemVer.
    let normalized = if tag == "0.1" { "0.1.0" } else { tag };
    Version::parse(normalized).map_err(|_| invalid_release())
}
fn invalid_release() -> ServiceError {
    ServiceError(
        ErrorCode::InvalidResponse,
        "发布信息格式不正确，请在 GitHub Releases 核对版本。",
    )
}
fn busy() -> ServiceError {
    ServiceError(ErrorCode::Conflict, "已有更新操作正在进行，请稍后重试。")
}
fn state(app: &tauri::AppHandle) -> backend::Result<std::sync::MutexGuard<'_, Pending>> {
    app.state::<UpdateState>()
        .inner()
        .0
        .lock()
        .map_err(|_| ServiceError(ErrorCode::InternalError, "更新服务需要重新启动。"))
}
fn backend(app: &tauri::AppHandle) -> backend::Result<Backend> {
    app.state::<BackendState>().0.clone()
}
fn asset_url(tag: &str, name: &str) -> String {
    format!("{REPOSITORY}/releases/download/{tag}/{name}")
}
fn asset<'a>(release: &'a Release, name: &str) -> backend::Result<Option<&'a str>> {
    let matches: Vec<_> = release
        .assets
        .iter()
        .filter(|asset| asset.name == name)
        .collect();
    if matches.len() > 1 {
        return Err(invalid_release());
    }
    if let Some(found) = matches.first() {
        if found.browser_download_url != asset_url(&release.tag_name, name) {
            return Err(invalid_release());
        }
        return Ok(Some(&found.browser_download_url));
    }
    Ok(None)
}
fn validate_release(release: &Release) -> backend::Result<Version> {
    let version = release_version(&release.tag_name)?;
    if release.draft
        || release.prerelease
        || !version.pre.is_empty()
        || release.html_url != format!("{REPOSITORY}/releases/tag/{}", release.tag_name)
        || release.tag_name.contains(['/', '?', '#', '%'])
    {
        return Err(invalid_release());
    }
    Ok(version)
}
fn fetch_release() -> backend::Result<Release> {
    let response = backend::network::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .and_then(|client| {
            client
                .get(RELEASE_API)
                .header("User-Agent", concat!("NekoBox/", env!("CARGO_PKG_VERSION")))
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .send()
        })
        .map_err(|_| {
            ServiceError(
                ErrorCode::NetworkUnavailable,
                "无法连接 GitHub，请检查网络和代理设置。",
            )
        })?;
    let status = response.status();
    if status.as_u16() == 403 || status.as_u16() == 429 {
        return Err(ServiceError(
            ErrorCode::RateLimited,
            "GitHub 请求次数受限，请稍后重试。",
        ));
    }
    if !status.is_success() {
        return Err(invalid_release());
    }
    let mut bytes = Vec::new();
    response
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_release())?;
    if bytes.len() > 1_048_576 {
        return Err(invalid_release());
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid_release())
}
async fn check(app: &tauri::AppHandle) -> backend::Result<UpdateStatus> {
    {
        let mut pending = state(app)?;
        if matches!(
            pending.status.phase.as_str(),
            "checking" | "downloading" | "ready" | "installing"
        ) {
            return Err(busy());
        }
        *pending = Pending::default();
        pending.status.phase = "checking".into();
        pending.status.message = "正在连接 GitHub…".into();
    }
    let result = check_inner(app).await;
    if let Err(error) = &result {
        let mut pending = state(app)?;
        pending.status.phase = "error".into();
        pending.status.message = error.1.into();
    }
    result
}
async fn check_inner(app: &tauri::AppHandle) -> backend::Result<UpdateStatus> {
    // Blocking HTTP never runs on the UI/runtime thread.
    let release = tauri::async_runtime::spawn_blocking(fetch_release)
        .await
        .map_err(|_| ServiceError(ErrorCode::InternalError, "更新检查失败，请重试。"))??;
    let version = validate_release(&release)?;
    let current = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|_| invalid_release())?;
    if version <= current {
        let mut pending = state(app)?;
        pending.status.phase = "current".into();
        pending.status.message = "当前已是最新稳定版本。".into();
        pending.status.release_url = release.html_url;
        return Ok(pending.status.clone());
    }
    let installer_name = format!("NekoBox_{version}_{}-setup.exe", architecture());
    let portable_name = format!("NekoBox_{version}_{}-portable.zip", architecture());
    let manual_url = asset(
        &release,
        if installation() == "installer" {
            &installer_name
        } else {
            &portable_name
        },
    )?
    .map(str::to_owned);
    let manifest = asset(&release, "latest.json")?;
    let update = if let Some(manifest) = manifest.filter(|_| installation() == "installer") {
        let builder = app
            .updater_builder()
            .timeout(Duration::from_secs(20))
            .configure_client(backend::network::configure_async)
            .endpoints(vec![manifest.parse().map_err(|_| invalid_release())?])
            .map_err(|_| invalid_release())?;
        let update = builder
            .build()
            .map_err(|_| invalid_release())?
            .check()
            .await
            .map_err(|_| {
                ServiceError(
                    ErrorCode::InvalidResponse,
                    "更新清单读取失败，请检查网络或查看 GitHub 发布页面。",
                )
            })?
            .ok_or_else(invalid_release)?;
        validate_update(
            &update.version,
            update.download_url.as_str(),
            &update.signature,
            &version.to_string(),
            &asset_url(&release.tag_name, &installer_name),
        )?;
        if asset(&release, &installer_name)?.is_none() {
            return Err(invalid_release());
        }
        Some(update)
    } else {
        None
    };
    let mut pending = state(app)?;
    pending.status.phase = "available".into();
    pending.status.version = Some(version.to_string());
    pending.status.notes = release.body.unwrap_or_default();
    pending.status.release_url = release.html_url;
    pending.status.download_url = manual_url;
    pending.status.signed = update.is_some();
    pending.status.message = if installation() == "portable" {
        "新版本已发布。便携版请下载后退出软件，替换程序并保留原有 data 目录。"
    } else if update.is_some() {
        "新版本已发布，可以下载并校验安装包。"
    } else {
        "该版本尚未提供签名更新清单，请通过发布页面手动升级。"
    }
    .into();
    pending.update = update;
    Ok(pending.status.clone())
}
fn validate_update(
    version: &str,
    url: &str,
    signature: &str,
    expected_version: &str,
    expected_url: &str,
) -> backend::Result<()> {
    if version != expected_version || url != expected_url || signature.trim().is_empty() {
        return Err(invalid_release());
    }
    Ok(())
}
async fn download(app: &tauri::AppHandle) -> backend::Result<UpdateStatus> {
    let mut update = {
        let mut pending = state(app)?;
        if pending.status.phase != "available" {
            return Err(busy());
        }
        let update = pending
            .update
            .clone()
            .ok_or_else(|| backend::invalid("此版本需要手动下载。"))?;
        pending.status.phase = "downloading".into();
        pending.status.downloaded = 0;
        pending.status.total = None;
        pending.status.message = "正在下载并校验签名…".into();
        update
    };
    // Large downloads have a longer bounded timeout; proxy policy is read again by configure_client.
    update.timeout = Some(Duration::from_secs(600));
    let result = update
        .download(
            |chunk, total| {
                if let Ok(mut pending) = state(app) {
                    pending.status.downloaded += chunk as u64;
                    pending.status.total = total;
                }
            },
            || {},
        )
        .await;
    let mut pending = state(app)?;
    match result {
        Ok(bytes) => {
            pending.bytes = Some(bytes);
            pending.status.phase = "ready".into();
            pending.status.message = "安装包签名已验证。安装将关闭软件，完成后重新打开。".into();
            Ok(pending.status.clone())
        }
        Err(_) => {
            pending.status.phase = "available".into();
            pending.status.message =
                "下载或签名验证失败，未执行安装。请重试或查看发布页面。".into();
            Err(ServiceError(
                ErrorCode::NetworkUnavailable,
                "下载或签名验证失败，未执行安装。",
            ))
        }
    }
}
fn install(app: &tauri::AppHandle, confirmed: bool) -> backend::Result<UpdateStatus> {
    if !confirmed {
        return Err(backend::invalid("安装更新需要确认关闭软件。"));
    }
    if installation() != "installer" {
        return Err(backend::invalid("便携版请手动更新。"));
    }
    let backend = backend(app)?;
    let _operations = backend.operation_gate.try_write().map_err(|_| {
        ServiceError(
            ErrorCode::Conflict,
            "仍有操作正在执行，请完成操作后安装更新。",
        )
    })?;
    let _launch = backend.launch_lock.try_lock().map_err(|_| busy())?;
    let _saves = backend.saves.try_lock().map_err(|_| busy())?;
    let _transfers = backend.transfers.try_lock().map_err(|_| busy())?;
    let backups = backend.backups.try_lock().map_err(|_| busy())?;
    let metadata = backend.metadata_refresh.try_lock().map_err(|_| busy())?;
    if backups.state.running || metadata.state.status == "running" || !backend.scans.is_idle()? {
        return Err(ServiceError(
            ErrorCode::Conflict,
            "扫描、资料更新或备份正在进行，请完成后安装更新。",
        ));
    }
    let db = backend.database()?;
    db.prepare_application_update()?;
    let mut pending = state(app)?;
    if pending.status.phase != "ready" {
        return Err(backend::invalid("请先下载并验证更新。"));
    }
    let update = pending.update.as_ref().ok_or_else(invalid_release)?;
    let bytes = pending.bytes.as_ref().ok_or_else(invalid_release)?;
    // All foreground operations and DB writes are held until the Windows updater exits.
    update.install(bytes).map_err(|_| {
        ServiceError(
            ErrorCode::InternalError,
            "安装程序启动失败，可以重试或手动安装。",
        )
    })?;
    pending.status.phase = "installing".into();
    Ok(pending.status.clone())
}
fn open(app: &tauri::AppHandle, download: bool) -> backend::Result<bool> {
    let pending = state(app)?;
    let url = if download {
        pending
            .status
            .download_url
            .as_deref()
            .ok_or_else(|| backend::invalid("该架构的下载包暂不可用，请查看发布页面。"))?
    } else {
        &pending.status.release_url
    };
    // URLs originate only from a validated release or the fixed repository constant.
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let value: Vec<u16> = std::ffi::OsStr::new(url)
            .encode_wide()
            .chain(Some(0))
            .collect();
        let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                value.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            return Err(backend::invalid("无法打开系统浏览器。"));
        }
    }
    #[cfg(target_os = "macos")]
    if !std::process::Command::new("/usr/bin/open")
        .arg(url)
        .status()
        .map_err(|_| backend::invalid("无法打开系统浏览器。"))?
        .success()
    {
        return Err(backend::invalid("无法打开系统浏览器。"));
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = url;
        return Err(backend::invalid("当前平台尚未接入外部浏览器。"));
    }
    #[allow(unreachable_code)]
    Ok(true)
}
fn response<T: Serialize>(id: String, result: backend::Result<T>) -> ApiResponse<T> {
    match result {
        Ok(data) => ApiResponse::ok(id, data, "操作完成。"),
        Err(error) => ApiResponse::error(id, error.0, error.1),
    }
}
macro_rules! validate_request {
    ($request:ident) => {
        if !valid_request_id(&$request.request_id) {
            return ApiResponse::error(String::new(), ErrorCode::InvalidRequest, "请求标识无效。");
        }
    };
}
#[tauri::command]
pub async fn check_app_update(
    app: tauri::AppHandle,
    request: ApiRequest<EmptyRequest>,
) -> ApiResponse<UpdateStatus> {
    validate_request!(request);
    response(request.request_id, check(&app).await)
}
#[tauri::command]
pub fn get_app_update_status(
    app: tauri::AppHandle,
    request: ApiRequest<EmptyRequest>,
) -> ApiResponse<UpdateStatus> {
    validate_request!(request);
    response(
        request.request_id,
        state(&app).map(|pending| pending.status.clone()),
    )
}
#[tauri::command]
pub async fn download_app_update(
    app: tauri::AppHandle,
    request: ApiRequest<EmptyRequest>,
) -> ApiResponse<UpdateStatus> {
    validate_request!(request);
    response(request.request_id, download(&app).await)
}
#[tauri::command]
pub async fn install_app_update(
    app: tauri::AppHandle,
    request: ApiRequest<InstallRequest>,
) -> ApiResponse<UpdateStatus> {
    validate_request!(request);
    let result =
        tauri::async_runtime::spawn_blocking(move || install(&app, request.payload.confirmed))
            .await
            .unwrap_or(Err(ServiceError(
                ErrorCode::InternalError,
                "更新安装失败。",
            )));
    response(request.request_id, result)
}
#[tauri::command]
pub async fn open_app_update_release(
    app: tauri::AppHandle,
    request: ApiRequest<OpenRequest>,
) -> ApiResponse<bool> {
    validate_request!(request);
    let result = tauri::async_runtime::spawn_blocking(move || open(&app, request.payload.download))
        .await
        .unwrap_or(Err(ServiceError(
            ErrorCode::InternalError,
            "无法打开发布页面。",
        )));
    response(request.request_id, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires live GitHub network access"]
    fn live_github_release_is_reachable_and_valid() {
        let release = fetch_release().expect("GitHub Release API must be reachable");
        let version = validate_release(&release).expect("stable repository release");
        println!(
            "Verified public GitHub release {} -> {}",
            release.tag_name, version
        );
    }
    #[test]
    fn semantic_version_comparison_and_legacy_tag() {
        assert_eq!(release_version("V0.1").unwrap(), Version::new(0, 1, 0));
        assert!(release_version("v0.1.10").unwrap() > release_version("v0.1.9").unwrap());
        for tag in ["latest", "v1", "v0.2", "v01.1.0", "v0.1.1/evil"] {
            assert!(release_version(tag).is_err());
        }
    }
    fn release() -> Release {
        Release {
            tag_name: "v0.1.2".into(),
            html_url: format!("{REPOSITORY}/releases/tag/v0.1.2"),
            body: None,
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                name: "latest.json".into(),
                browser_download_url: asset_url("v0.1.2", "latest.json"),
            }],
        }
    }
    #[test]
    fn only_stable_releases_from_this_repository_are_accepted() {
        let mut release = release();
        assert!(validate_release(&release).is_ok());
        release.prerelease = true;
        assert!(validate_release(&release).is_err());
        release.prerelease = false;
        release.html_url = "https://github.com/other/repo/releases/tag/v0.1.2".into();
        assert!(validate_release(&release).is_err());
    }
    #[test]
    fn assets_must_be_unique_and_version_pinned() {
        let mut release = release();
        assert!(asset(&release, "latest.json").unwrap().is_some());
        assert!(asset(&release, "missing.exe").unwrap().is_none());
        release.assets[0].browser_download_url =
            format!("{REPOSITORY}/releases/latest/download/latest.json");
        assert!(asset(&release, "latest.json").is_err());
        release.assets[0].browser_download_url = asset_url("v0.1.2", "latest.json");
        release.assets.push(Asset {
            name: "latest.json".into(),
            browser_download_url: asset_url("v0.1.2", "latest.json"),
        });
        assert!(asset(&release, "latest.json").is_err());
    }
    #[test]
    fn updater_metadata_cannot_switch_architecture_version_or_repository() {
        let url = asset_url("v0.1.2", "NekoBox_0.1.2_x64-setup.exe");
        assert!(validate_update("0.1.2", &url, "signature", "0.1.2", &url).is_ok());
        assert!(validate_update("0.1.1", &url, "signature", "0.1.2", &url).is_err());
        assert!(validate_update(
            "0.1.2",
            &url.replace("x64", "arm64"),
            "signature",
            "0.1.2",
            &url
        )
        .is_err());
        assert!(validate_update(
            "0.1.2",
            &url.replace("doudou0611", "other"),
            "signature",
            "0.1.2",
            &url
        )
        .is_err());
        assert!(validate_update("0.1.2", &url, "", "0.1.2", &url).is_err());
    }
}

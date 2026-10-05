use super::*;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};
#[derive(Clone, Serialize, Deserialize)]
pub struct Download {
    pub id: String,
    pub game_id: String,
    pub title: String,
    pub status: String,
    pub message: String,
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64,
    pub install_path: String,
}
impl Download {
    pub(super) fn active(&self) -> bool {
        matches!(self.status.as_str(), "queued" | "running")
    }
}
#[derive(Default)]
pub struct Manager {
    pub tasks: Vec<Download>,
    cancel: HashMap<String, Arc<AtomicBool>>,
    loaded: bool,
}
#[derive(Deserialize)]
pub struct DownloadRequest {
    pub game_id: String,
    #[serde(default)]
    pub depot: Option<String>,
}
#[derive(Deserialize)]
pub struct TaskRequest {
    pub task_id: String,
}
type Chunk = (u64, u64, u64, String);
type ManifestFile = (String, u64, u64, u64, String, Vec<Chunk>);
#[derive(Deserialize)]
struct Manifest {
    build_id: u64,
    filename: String,
    #[serde(default)]
    depot: String,
    files: Files,
}
#[derive(Default, Deserialize)]
struct Files {
    #[serde(default)]
    add: Vec<ManifestFile>,
    #[serde(default)]
    update: Vec<ManifestFile>,
    #[serde(default)]
    delete: Vec<String>,
}
fn io_error() -> ServiceError {
    permission("游戏文件无法写入，请检查磁盘空间、目录权限或文件占用后重试。")
}
fn cancelled() -> ServiceError {
    ServiceError(
        ErrorCode::Cancelled,
        "下载已取消，已校验的数据块保留，可再次下载继续。",
    )
}
fn active(flag: &AtomicBool) -> Result<()> {
    if flag.load(Ordering::Relaxed) {
        Err(cancelled())
    } else {
        Ok(())
    }
}
fn load(b: &Backend, m: &mut Manager) -> Result<()> {
    if !m.loaded {
        m.tasks = b.database()?.setting(JOBS)?.unwrap_or_default();
        for t in &mut m.tasks {
            if t.active() {
                t.status = "cancelled".into();
                t.message = "上次下载已中断，再次下载将复用已校验数据块。".into();
                t.speed = 0;
            }
        }
        m.loaded = true;
        b.database()?.put_setting(JOBS, &m.tasks)?;
    }
    Ok(())
}
pub fn downloads(b: &Backend) -> Result<Vec<Download>> {
    let mut m = b.hikarifield_downloads.lock().map_err(|_| bad())?;
    load(b, &mut m)?;
    Ok(m.tasks.clone())
}
pub fn cancel(b: &Backend, q: TaskRequest) -> Result<bool> {
    let mut m = b.hikarifield_downloads.lock().map_err(|_| bad())?;
    if let Some(flag) = m.cancel.get(&q.task_id) {
        flag.store(true, Ordering::Relaxed);
    }
    if let Some(t) = m.tasks.iter_mut().find(|t| t.id == q.task_id && t.active()) {
        if t.status == "queued" {
            t.status = "cancelled".into();
            t.message = "下载已取消。".into();
        } else {
            t.message = "正在取消下载…".into();
        }
    }
    b.database()?.put_setting(JOBS, &m.tasks)?;
    Ok(true)
}
fn progress(b: &Backend, id: &str, update: impl FnOnce(&mut Download)) -> Result<()> {
    let mut m = b.hikarifield_downloads.lock().map_err(|_| bad())?;
    if let Some(t) = m.tasks.iter_mut().find(|t| t.id == id) {
        update(t);
    }
    b.database()?.put_setting(JOBS, &m.tasks)?;
    Ok(())
}
/// Reject Windows traversal syntax even when running tests on macOS.
fn relative(value: &str) -> Result<PathBuf> {
    let value = value.replace('\\', "/");
    if value.is_empty()
        || value.len() > 2048
        || value.starts_with('/')
        || value
            .chars()
            .any(|c| c.is_control() || ":<>\"|?*".contains(c))
    {
        return Err(invalid("HIKARI FIELD 文件路径无效。"));
    }
    let mut out = PathBuf::new();
    for part in value.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.ends_with(['.', ' ']) {
            return Err(invalid("HIKARI FIELD 文件路径超出安装目录。"));
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.as_bytes()[3].is_ascii_digit())
        {
            return Err(invalid("HIKARI FIELD 文件名不受支持。"));
        }
        out.push(part);
    }
    Ok(out)
}
fn safe_path(root: &Path, value: &str) -> Result<PathBuf> {
    let rel = relative(value)?;
    let mut p = root.to_path_buf();
    for part in rel.components() {
        p.push(part);
        if fs::symlink_metadata(&p).is_ok_and(|m| scanner::linked(&m)) {
            return Err(invalid("游戏目录中存在符号链接，已停止写入。"));
        }
    }
    Ok(p)
}
fn checked_root(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if fs::symlink_metadata(path).is_ok_and(|m| scanner::linked(&m)) {
        return Err(invalid("安装根目录不能是符号链接。"));
    }
    let canonical = absolute_directory(value)?;
    if canonical != path {
        return Err(invalid("安装根目录已变更，请在设置中重新选择。"));
    }
    Ok(canonical)
}
fn sha1(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let mut digest = ring::digest::Context::new(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY);
    let mut buf = [0; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    Ok(digest
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
fn valid_file(path: &Path, size: u64, hash: &str) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == size)
        && hash_file(path).is_ok_and(|h| h.eq_ignore_ascii_case(hash))
}
fn valid_hash(hash: &str) -> bool {
    hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}
fn validate_manifest(m: &Manifest, build: u64) -> Result<()> {
    if m.build_id != build
        || m.files.add.len() + m.files.update.len() > 100000
        || m.filename.is_empty()
    {
        return Err(bad());
    }
    relative(&m.filename)?;
    let mut names = std::collections::HashSet::new();
    for (name, size, _, _, hash, chunks) in m.files.add.iter().chain(&m.files.update) {
        let rel = relative(name)?;
        if rel
            .iter()
            .next()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("_temp"))
            || name.eq_ignore_ascii_case("install.json")
        {
            return Err(bad());
        }
        if !names.insert(name.replace('\\', "/").to_lowercase())
            || !valid_hash(hash)
            || *size > 1024 * 1024 * 1024 * 1024
            || chunks.len() > 100000
        {
            return Err(bad());
        }
        let mut sum = 0u64;
        for (len, start, end, hash) in chunks {
            if !valid_hash(hash)
                || end < start
                || end - start > 512 * 1024 * 1024
                || *len > 512 * 1024 * 1024
            {
                return Err(bad());
            }
            sum = sum.checked_add(*len).ok_or_else(bad)?;
        }
        if sum != *size {
            return Err(bad());
        }
    }
    for path in &m.files.delete {
        let rel = relative(path)?;
        if rel
            .iter()
            .next()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("_temp"))
            || path.eq_ignore_ascii_case("install.json")
        {
            return Err(bad());
        }
    }
    Ok(())
}
fn url_from(v: Value, filename: &str, account_id: u64) -> Result<(String, i64)> {
    let a = v.as_array().filter(|a| a.len() == 4).ok_or_else(bad)?;
    let mut url = reqwest::Url::parse(a[0].as_str().ok_or_else(bad)?).map_err(|_| bad())?;
    let host = url.host_str().unwrap_or_default();
    #[cfg(test)]
    let local_fixture = url.scheme() == "http" && url.host_str() == Some("127.0.0.1");
    #[cfg(not(test))]
    let local_fixture = false;
    if !local_fixture
        && (url.scheme() != "https"
            || !(host == "download.hikarifield.co.jp"
                || host.ends_with(".hikarifield.co.jp")
                || host.ends_with(".cloudfront.net"))
            || url.port_or_known_default() != Some(443)
            || !url.username().is_empty()
            || url.password().is_some())
    {
        return Err(bad());
    }
    let expires = a[1]
        .as_i64()
        .or_else(|| a[1].as_str()?.parse().ok())
        .ok_or_else(bad)?;
    if expires <= chrono::Utc::now().timestamp() {
        return Err(bad());
    }
    let path = format!(
        "{}/{}",
        url.path().trim_end_matches('/'),
        filename.replace('\\', "/")
    );
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    url.query_pairs_mut()
        .append_pair("uid", &account_id.to_string())
        .append_pair("Expires", &expires.to_string())
        .append_pair("Signature", a[3].as_str().ok_or_else(bad)?)
        .append_pair("Key-Pair-Id", a[2].as_str().ok_or_else(bad)?);
    Ok((url.into(), expires))
}
pub fn start(b: &Backend, q: DownloadRequest) -> Result<Download> {
    let ownership = b.database()?.hf_ownership(&q.game_id)?;
    let (account_id, value) = request_identity(b, "apps", json!({"category_id":3}))?;
    let mut app = parse_apps(value)?
        .into_iter()
        .find(|a| a.id == ownership.app.id && a.have == 1 && a.trial == 0 && a.asmr == 0)
        .ok_or_else(|| permission("当前账户未拥有此游戏，请登录购买它的账户。"))?;
    if app.released != 1 {
        return Err(invalid("此游戏尚未在 HIKARI FIELD 开放下载。"));
    }
    let _account = b.hikarifield_lock.lock().map_err(|_| bad())?;
    if read_credential(b)?.account_id != account_id {
        return Err(invalid("账户已变更，请重新下载。"));
    }
    // Serialize registration with launch so a game cannot start while its files change.
    let _launch = b.launch_lock.lock().map_err(|_| bad())?;
    b.database()?.get_game(&q.game_id)?;
    if b.database()?.has_open_session(&q.game_id)? {
        return Err(invalid("请先关闭游戏，再下载或修复文件。"));
    }
    let config = settings(b)?;
    let root = checked_root(
        config
            .root
            .as_deref()
            .ok_or_else(|| invalid("首次下载请先选择 HikariFieldGames 的存放位置。"))?,
    )?;
    let depot = q.depot.unwrap_or_else(|| {
        if app.depots.contains_key("main") {
            "main".into()
        } else {
            app.depots.keys().next().cloned().unwrap_or("main".into())
        }
    });
    if let Some(d) = app.depots.get(&depot) {
        app.build_id = d.build_id;
        app.version = d.version.clone();
    } else if !app.depots.is_empty() {
        return Err(invalid("请选择有效的游戏版本。"));
    }
    if app.build_id == 0 || app.exec_file.is_empty() {
        return Err(bad());
    }
    let install_path = safe_path(&root, &app.install_path)?;
    relative(&app.exec_file)?;
    let mut m = b.hikarifield_downloads.lock().map_err(|_| bad())?;
    load(b, &mut m)?;
    if m.tasks.iter().any(|t| {
        t.active()
            && (t.game_id == q.game_id
                || t.install_path == path_text(&install_path).unwrap_or_default())
    }) {
        return Err(invalid("此游戏已有下载任务。"));
    }
    if m.tasks.iter().filter(|t| t.active()).count() >= 20 {
        return Err(invalid("下载队列已满，请稍后添加。"));
    }
    let task = Download {
        id: id(),
        game_id: q.game_id,
        title: app.name.clone(),
        status: "queued".into(),
        message: "等待下载".into(),
        downloaded: 0,
        total: 0,
        speed: 0,
        install_path: path_text(&install_path)?,
    };
    let flag = Arc::new(AtomicBool::new(false));
    m.cancel.insert(task.id.clone(), flag.clone());
    m.tasks.insert(0, task.clone());
    while m.tasks.len() > 60 {
        if let Some(i) = m.tasks.iter().rposition(|t| !t.active()) {
            m.tasks.remove(i);
        } else {
            break;
        }
    }
    b.database()?.put_setting(JOBS, &m.tasks)?;
    drop(m);
    let backend = b.clone();
    let job = task.clone();
    std::thread::spawn(move || {
        let run = || -> Result<()> {
            let _worker = backend.hikarifield_worker.lock().map_err(|_| bad())?;
            active(&flag)?;
            let _gate = backend.operation_gate.read().map_err(|_| bad())?;
            run_download(&backend, &job, &app, &config, account_id, &flag)
        };
        let result = run();
        let _ = progress(&backend, &job.id, |t| {
            t.speed = 0;
            match &result {
                Ok(_) => {
                    t.status = "completed".into();
                    t.message = "安装完成，可以启动游戏。".into();
                    t.downloaded = t.total;
                }
                Err(e) => {
                    t.status = if e.0 == ErrorCode::Cancelled {
                        "cancelled"
                    } else {
                        "failed"
                    }
                    .into();
                    t.message = e.1.into();
                }
            }
        });
        if let Ok(mut m) = backend.hikarifield_downloads.lock() {
            m.cancel.remove(&job.id);
        }
    });
    Ok(task)
}
struct Meter<'a> {
    api: &'a dyn Fn(&str, Value) -> Result<Value>,
    sample_bytes: u64,
    b: &'a Backend,
    job: &'a Download,
    app: &'a App,
    done: u64,
    total: u64,
    delta: u64,
    last: Instant,
    stats: Instant,
}
impl Meter<'_> {
    fn tick(&mut self, n: u64, network: bool) -> Result<()> {
        self.done = self.done.saturating_add(n);
        if network {
            self.sample_bytes = self.sample_bytes.saturating_add(n);
            self.delta = self.delta.saturating_add(n);
        }
        if self.last.elapsed().as_millis() >= 400 {
            let speed = if network {
                (self.sample_bytes as f64 / self.last.elapsed().as_secs_f64()) as u64
            } else {
                0
            };
            progress(self.b, &self.job.id, |t| {
                t.downloaded = self.done;
                t.total = self.total;
                t.speed = speed;
            })?;
            self.last = Instant::now();
            self.sample_bytes = 0;
        }
        if self.stats.elapsed().as_secs() >= 30 {
            self.flush();
        }
        Ok(())
    }
    fn flush(&mut self) {
        if self.delta > 0 {
            let delta = self.delta;
            let seconds = self.stats.elapsed().as_secs().max(1);
            let _ = (self.api)(
                "builds/bytes",
                json!({"game_build_id":self.app.build_id,"task_type":0,"bytes_total":self.total,"bytes_downloaded":delta,"download_speed":delta/seconds}),
            );
            self.delta = 0;
        }
        self.stats = Instant::now();
    }
}
impl Drop for Meter<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}
fn run_download(
    b: &Backend,
    job: &Download,
    app: &App,
    config: &Settings,
    account_id: u64,
    flag: &AtomicBool,
) -> Result<()> {
    run_download_with(b, job, app, config, account_id, flag, &|path, body| {
        request(b, path, body)
    })
}
#[allow(clippy::too_many_arguments)]
fn run_download_with(
    b: &Backend,
    job: &Download,
    app: &App,
    config: &Settings,
    account_id: u64,
    flag: &AtomicBool,
    api: &impl Fn(&str, Value) -> Result<Value>,
) -> Result<()> {
    let root = checked_root(config.root.as_deref().ok_or_else(bad)?)?;
    let dir = safe_path(&root, &app.install_path)?;
    fs::create_dir_all(&dir).map_err(|_| io_error())?;
    let temp = safe_path(&dir, "_temp")?;
    fs::create_dir_all(&temp).map_err(|_| io_error())?;
    progress(b, &job.id, |t| {
        t.status = "running".into();
        t.message = "正在获取文件清单…".into();
    })?;
    let manifest: Manifest = serde_json::from_value(api(
        "files/",
        json!({"game_build_id":app.build_id,"from_build_id":0}),
    )?)
    .map_err(|_| bad())?;
    validate_manifest(&manifest, app.build_id)?;
    active(flag)?;
    let total = manifest
        .files
        .add
        .iter()
        .chain(&manifest.files.update)
        .flat_map(|f| &f.5)
        .try_fold(0u64, |n, c| n.checked_add(c.2 - c.1 + 1))
        .ok_or_else(bad)?;
    let mut meter = Meter {
        api,
        sample_bytes: 0,
        b,
        job,
        app,
        done: 0,
        total,
        delta: 0,
        last: Instant::now(),
        stats: Instant::now(),
    };
    let result = (|| -> Result<()> {
        let mut signed: Option<(String, i64)> = None;
        let http = http()?;
        for (name, size, _, _, hash, chunks) in
            manifest.files.add.iter().chain(&manifest.files.update)
        {
            active(flag)?;
            let target = safe_path(&dir, name)?;
            if valid_file(&target, *size, hash) {
                for c in chunks {
                    meter.tick(c.2 - c.1 + 1, false)?;
                }
                continue;
            }
            let mut parts = vec![];
            for (index, (len, start, end, chunk_hash)) in chunks.iter().enumerate() {
                active(flag)?;
                let chunk_name = sha1(format!("{name}_{}", index + 1).as_bytes());
                let part = safe_path(&temp, &chunk_name)?;
                parts.push(part.clone());
                if valid_file(&part, *len, chunk_hash) {
                    meter.tick(end - start + 1, false)?;
                    continue;
                }
                progress(b, &job.id, |t| t.message = "正在下载游戏…".into())?;
                if signed
                    .as_ref()
                    .is_none_or(|(_, expires)| *expires <= chrono::Utc::now().timestamp() + 10)
                {
                    signed = Some(url_from(
                        api(
                            "builds/sign",
                            json!({"game_build_id":app.build_id,"task_type":0,"uuid":config.uuid}),
                        )?,
                        &manifest.filename,
                        account_id,
                    )?);
                }
                let (url, _) = signed.as_ref().ok_or_else(bad)?;
                let mut response = http
                    .get(url)
                    .header("Range", format!("bytes={start}-{end}"))
                    .send()
                    .map_err(|_| net())?;
                if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                    return Err(response_error(response.status().as_u16()));
                }
                let range = response
                    .headers()
                    .get("content-range")
                    .and_then(|h| h.to_str().ok())
                    .ok_or_else(bad)?;
                if !range.starts_with(&format!("bytes {start}-{end}/")) {
                    return Err(bad());
                }
                let zip_path = safe_path(&temp, &format!("{chunk_name}.zip"))?;
                let mut writer = File::create(&zip_path).map_err(|_| io_error())?;
                let mut count = 0u64;
                let mut buf = [0u8; 65536];
                loop {
                    active(flag)?;
                    let n = response.read(&mut buf).map_err(|_| net())?;
                    if n == 0 {
                        break;
                    }
                    count += n as u64;
                    if count > end - start + 1 {
                        return Err(bad());
                    }
                    writer.write_all(&buf[..n]).map_err(|_| io_error())?;
                    meter.tick(n as u64, true)?;
                }
                writer.sync_all().map_err(|_| io_error())?;
                drop(writer);
                if count != end - start + 1 {
                    return Err(net());
                }
                extract_part(&zip_path, &part, &chunk_name, *len, chunk_hash, flag)?;
                let _ = fs::remove_file(zip_path);
            }
            progress(b, &job.id, |t| t.message = "正在合并与校验文件…".into())?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|_| io_error())?;
            }
            let stage = safe_path(&temp, &format!("{}.installing", sha1(name.as_bytes())))?;
            let mut output = File::create(&stage).map_err(|_| io_error())?;
            for part in &parts {
                let mut input = File::open(part).map_err(|_| io_error())?;
                let mut buf = [0; 65536];
                loop {
                    active(flag)?;
                    let n = input.read(&mut buf).map_err(|_| io_error())?;
                    if n == 0 {
                        break;
                    }
                    output.write_all(&buf[..n]).map_err(|_| io_error())?;
                }
            }
            output.sync_all().map_err(|_| io_error())?;
            drop(output);
            if !valid_file(&stage, *size, hash) {
                return Err(ServiceError(
                    ErrorCode::InvalidResponse,
                    "游戏文件校验失败，请重试下载。",
                ));
            }
            safe_path(&dir, name)?;
            fs::rename(&stage, &target).map_err(|_| io_error())?;
            for part in parts {
                let _ = fs::remove_file(part);
            }
        }
        active(flag)?;
        for name in &manifest.files.delete {
            let path = safe_path(&dir, name)?;
            if path.is_file() {
                fs::remove_file(path).map_err(|_| io_error())?;
            }
        }
        let exe = safe_path(&dir, &app.exec_file)?;
        if !exe.is_file() {
            return Err(invalid("下载完成但未找到启动文件，请检查游戏版本。"));
        }
        let install = json!({app.tag.clone():{"build_id":app.build_id,"version":app.version,"depot":manifest.depot,"installed_path":path_text(&dir)?,"exec_file":app.exec_file}});
        let info = safe_path(&dir, "install.json")?;
        fs::write(
            info,
            serde_json::to_vec_pretty(&install).map_err(|_| bad())?,
        )
        .map_err(|_| io_error())?;
        b.database()?
            .attach_hf_installation(&job.game_id, &path_text(&dir)?, &path_text(&exe)?)?;
        Ok(())
    })();
    meter.flush();
    let _ = api(
        "apps/installed",
        json!({"game_app_id":app.id,"game_build_id":app.build_id,"task_type":0,"task_status":match &result{Ok(_)=>2,Err(e) if e.0==ErrorCode::Cancelled=>4,_=>3},"uuid":config.uuid}),
    );
    result
}
fn extract_part(
    zip_path: &Path,
    part: &Path,
    name: &str,
    size: u64,
    hash: &str,
    flag: &AtomicBool,
) -> Result<()> {
    let mut archive =
        zip::ZipArchive::new(File::open(zip_path).map_err(|_| io_error())?).map_err(|_| bad())?;
    if archive.len() != 1 {
        return Err(bad());
    }
    let mut entry = archive.by_index(0).map_err(|_| bad())?;
    if entry.name() != name
        || entry.is_dir()
        || entry.size() != size
        || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
    {
        return Err(bad());
    }
    let stage = safe_path(
        part.parent().ok_or_else(bad)?,
        &format!("{name}.extracting"),
    )?;
    let mut out = File::create(&stage).map_err(|_| io_error())?;
    let mut count = 0;
    let mut buf = [0; 65536];
    loop {
        active(flag)?;
        let n = entry.read(&mut buf).map_err(|_| bad())?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > size {
            return Err(bad());
        }
        out.write_all(&buf[..n]).map_err(|_| io_error())?;
    }
    out.sync_all().map_err(|_| io_error())?;
    drop(out);
    if !valid_file(&stage, size, hash) {
        return Err(ServiceError(
            ErrorCode::InvalidResponse,
            "下载数据块校验失败，请重试。",
        ));
    }
    fs::rename(stage, part).map_err(|_| io_error())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn zip_chunk(name: &str, bytes: &[u8]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn range_download_resumes_verified_chunks_installs_and_registers_launch_path() {
        use std::net::TcpListener;
        let fixture = std::env::temp_dir().join(format!("nekobox-hf-{}", id()));
        let b = Backend::open(fixture.join("data")).unwrap();
        let root = fixture.join("HikariFieldGames");
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let app:App=serde_json::from_value(json!({"id":7,"tag":"fixture","name":"离线下载测试","have":1,"released":1,"build_id":11,"version":"1.0","install_path":"Fixture","exec_file":"Fixture.exe"})).unwrap();
        b.database().unwrap().import_hf_app(10, &app, None).unwrap();
        let q = crate::domain::models::GameQuery {
            page: 1,
            page_size: 100,
            search: String::new(),
            statuses: vec![],
            sources: vec![],
            tag_ids: vec![],
            favorite: None,
            collection_id: None,
            sort: crate::domain::models::GameSort::Title,
            direction: crate::domain::models::SortDirection::Asc,
            filters: Default::default(),
        };
        let game_id = b.database().unwrap().list_games(&q).unwrap().items[0]
            .id
            .clone();
        let first = b"MZ-fixture-first";
        let second = b"fixture-second";
        let content = [first.as_slice(), second.as_slice()].concat();
        let name1 = sha1(b"Fixture.exe_1");
        let name2 = sha1(b"Fixture.exe_2");
        let z1 = zip_chunk(&name1, first);
        let z2 = zip_chunk(&name2, second);
        let dir = root.join("Fixture");
        fs::create_dir_all(dir.join("_temp")).unwrap();
        fs::write(dir.join("_temp").join(&name1), first).unwrap();
        let start = z1.len() as u64;
        let end = start + z2.len() as u64 - 1;
        let manifest = json!({"build_id":11,"filename":"game.pack","depot":"main","files":{"add":[["Fixture.exe",content.len(),0,end,sha1(&content),[[first.len(),0,start-1,sha1(first)],[second.len(),start,end,sha1(second)]]]],"update":[],"delete":[]}});
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut buf = [0; 8192];
            let n = stream.read(&mut buf).unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.starts_with("GET /game.pack?uid=10&Expires="));
            assert!(req
                .to_lowercase()
                .contains(&format!("range: bytes={start}-{end}")));
            write!(stream,"HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nConnection: close\r\n\r\n",z2.len(),end+1).unwrap();
            stream.write_all(&z2).unwrap();
        });
        let calls = std::cell::RefCell::new(Vec::new());
        let api = |path: &str, body: Value| -> Result<Value> {
            calls.borrow_mut().push((path.to_string(), body));
            match path {
                "files/" => Ok(manifest.clone()),
                "builds/sign" => Ok(json!([
                    format!("http://{addr}"),
                    chrono::Utc::now().timestamp() + 600,
                    "fixture-key",
                    "fixture-signature"
                ])),
                _ => Ok(json!({"ok":true})),
            }
        };
        let job = Download {
            id: id(),
            game_id: game_id.clone(),
            title: app.name.clone(),
            status: "queued".into(),
            message: "".into(),
            downloaded: 0,
            total: 0,
            speed: 0,
            install_path: dir.to_string_lossy().into(),
        };
        {
            let mut m = b.hikarifield_downloads.lock().unwrap();
            m.loaded = true;
            m.tasks.push(job.clone());
        }
        let config = Settings {
            root: Some(root.to_string_lossy().into()),
            uuid: id(),
        };
        run_download_with(&b, &job, &app, &config, 10, &AtomicBool::new(false), &api).unwrap();
        worker.join().unwrap();
        assert_eq!(fs::read(dir.join("Fixture.exe")).unwrap(), content);
        let detail = b.database().unwrap().get_game(&game_id).unwrap();
        assert_eq!(detail.summary.installations.len(), 1);
        let install = &detail.summary.installations[0];
        assert!(install.path_valid);
        let launch = crate::domain::requests::LaunchGameRequest {
            install_id: install.id.clone(),
            options: crate::domain::requests::LaunchOptions {
                user_initiated: true,
            },
        };
        // Download registration prevents launch before the worker marks completion.
        assert_eq!(
            b.launch(&launch, "hf-launch-guard", None).unwrap_err().1,
            "请等待 HIKARI FIELD 下载完成，再启动游戏。"
        );
        assert_eq!(
            b.remove_record(&game_id, false).unwrap_err().1,
            "请先完成或取消此游戏的下载，再移除库记录。"
        );
        assert_eq!(
            install.executable_path.as_deref(),
            dir.join("Fixture.exe").to_str()
        );
        let install_json: Value =
            serde_json::from_slice(&fs::read(dir.join("install.json")).unwrap()).unwrap();
        assert_eq!(install_json["fixture"]["build_id"], 11);
        let requests = calls.borrow();
        assert_eq!(
            requests.iter().filter(|(p, _)| p == "builds/sign").count(),
            1
        );
        assert_eq!(
            requests
                .iter()
                .find(|(p, _)| p == "builds/bytes")
                .unwrap()
                .1["bytes_downloaded"],
            end - start + 1
        );
        assert_eq!(
            requests
                .iter()
                .find(|(p, _)| p == "apps/installed")
                .unwrap()
                .1["task_status"],
            2
        );
        drop(requests);
        calls.borrow_mut().clear();
        // A second run finds fully installed verified files and never needs a CDN signature.
        run_download_with(&b, &job, &app, &config, 10, &AtomicBool::new(false), &api).unwrap();
        assert!(!calls.borrow().iter().any(|(p, _)| p == "builds/sign"));
        assert_eq!(
            b.database()
                .unwrap()
                .get_game(&game_id)
                .unwrap()
                .summary
                .installations
                .len(),
            1
        );
        drop(b);
        fs::remove_dir_all(fixture).unwrap();
    }
    #[test]
    fn traversal_symlinks_zip_contents_and_hash_corruption_are_rejected() {
        for path in [
            "../evil.exe",
            "/evil",
            "C:\\evil",
            "..\\evil",
            "a/../b",
            "a.",
            "CON.exe",
            "a:b",
            "\\\\server\\share",
        ] {
            assert!(relative(path).is_err(), "{path}");
        }
        let root = std::env::temp_dir().join(format!("hf-safe-{}", id()));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(std::env::temp_dir(), root.join("escape")).unwrap();
            assert!(safe_path(&root, "escape/evil").is_err());
        }
        let name = sha1(b"file_1");
        let path = root.join("part.zip");
        let part = root.join(&name);
        fs::write(&path, zip_chunk("../evil", b"data")).unwrap();
        assert!(extract_part(
            &path,
            &part,
            &name,
            4,
            &sha1(b"data"),
            &AtomicBool::new(false)
        )
        .is_err());
        fs::write(&path, zip_chunk(&name, b"bad!")).unwrap();
        assert!(extract_part(
            &path,
            &part,
            &name,
            4,
            &sha1(b"data"),
            &AtomicBool::new(false)
        )
        .is_err());
        assert!(!part.exists());
        fs::write(&path, zip_chunk(&name, b"data")).unwrap();
        assert_eq!(
            extract_part(
                &path,
                &part,
                &name,
                4,
                &sha1(b"data"),
                &AtomicBool::new(true)
            )
            .unwrap_err()
            .0,
            ErrorCode::Cancelled
        );
        assert!(!part.exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interrupted_jobs_are_recoverable_and_terminal_without_automatic_download() {
        let fixture = std::env::temp_dir().join(format!("hf-recovery-{}", id()));
        let b = Backend::open(fixture.clone()).unwrap();
        let task = Download {
            id: "job".into(),
            game_id: "game".into(),
            title: "game".into(),
            status: "running".into(),
            message: "old".into(),
            downloaded: 10,
            total: 20,
            speed: 5,
            install_path: "path".into(),
        };
        b.database()
            .unwrap()
            .put_setting(JOBS, &vec![task])
            .unwrap();
        let tasks = downloads(&b).unwrap();
        assert_eq!(tasks[0].status, "cancelled");
        assert_eq!(tasks[0].speed, 0);
        assert!(!tasks[0].active());
        drop(b);
        fs::remove_dir_all(fixture).unwrap();
    }
}

//! WebDAV snapshots: unique objects, temporary upload + non-overwriting MOVE.
use super::*;
use super::{
    application_backup::{self, Backup, Config},
    credential::{NativeVault, Vault},
};
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, time::Duration};
#[derive(Deserialize)]
pub struct LocalRequest {
    pub backup_id: String,
}
#[derive(Deserialize)]
pub struct RemoteRequest {
    pub file_name: String,
    pub password: Option<String>,
}
#[derive(Serialize)]
pub struct RemoteBackup {
    pub file_name: String,
}
pub fn base(c: &Config) -> Result<reqwest::Url> {
    let mut url =
        reqwest::Url::parse(&c.webdav_url).map_err(|_| invalid("请输入有效的 WebDAV 地址。"))?;
    if !["http", "https"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid("WebDAV 地址只接受不含账号的 HTTP/HTTPS 地址。"));
    }
    let prefix = url.path().trim_end_matches('/').to_owned();
    url.set_path(&format!("{prefix}/"));
    let parts = c.webdav_directory.trim_matches('/');
    if !parts.is_empty() {
        if !super::saves::files::valid_name(parts) {
            return Err(invalid("WebDAV 目录应为相对目录，不包含上级路径。"));
        }
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| invalid("WebDAV 目录无效。"))?;
            segments.pop_if_empty();
            for p in parts.split('/') {
                segments.push(p);
            }
            segments.push("");
        }
    }
    Ok(url)
}
fn file_name(name: &str) -> Result<()> {
    let Some(id) = name
        .strip_prefix("GalgameManager-")
        .and_then(|s| s.strip_suffix(".gmbak"))
    else {
        return Err(invalid("云备份文件名无效。"));
    };
    uuid::Uuid::parse_str(id).map_err(|_| invalid("云备份标识无效。"))?;
    Ok(())
}
fn config(b: &Backend) -> Result<Config> {
    b.database()?
        .setting(application_backup::KEY)?
        .ok_or_else(|| invalid("请先保存 WebDAV 配置。"))
}
fn client() -> Result<reqwest::blocking::Client> {
    super::network::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| invalid("云连接初始化失败。"))
}
fn request(
    b: &Backend,
    c: &Config,
    method: reqwest::Method,
    url: reqwest::Url,
) -> Result<reqwest::blocking::RequestBuilder> {
    let password = if c.webdav_username.is_empty() {
        None
    } else {
        let raw = NativeVault::new(b, "dev.galgame.manager.webdav")?.read()?;
        if let Some(raw) = raw {
            let value: serde_json::Value = serde_json::from_str(&raw)
                .map_err(|_| invalid("WebDAV 凭据格式无效，请重新保存密码。"))?;
            if value["url"].as_str() != Some(c.webdav_url.as_str())
                || value["username"].as_str() != Some(c.webdav_username.as_str())
            {
                return Err(invalid("WebDAV 密码不属于当前地址或账户，请重新输入。"));
            }
            Some(
                value["password"]
                    .as_str()
                    .ok_or_else(|| invalid("WebDAV 密码无效。"))?
                    .to_owned(),
            )
        } else {
            None
        }
    };
    let r = client()?.request(method, url);
    Ok(if !c.webdav_username.is_empty() || password.is_some() {
        r.basic_auth(&c.webdav_username, password)
    } else {
        r
    })
}
fn send(r: reqwest::blocking::RequestBuilder) -> Result<reqwest::blocking::Response> {
    let response = r.send().map_err(|_| {
        ServiceError(
            ErrorCode::NetworkUnavailable,
            "WebDAV 请求失败，请检查地址、代理和网络。",
        )
    })?;
    if !response.status().is_success() {
        return Err(ServiceError(
            ErrorCode::NetworkUnavailable,
            "WebDAV 返回错误，请检查权限、目录和账号。",
        ));
    }
    Ok(response)
}
fn bounded(response: reqwest::blocking::Response, limit: u64) -> Result<Vec<u8>> {
    if response.content_length().is_some_and(|n| n > limit) {
        return Err(invalid("云端响应超过大小限制。"));
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("云端下载中断。"))?;
    if bytes.len() as u64 > limit {
        return Err(invalid("云端响应超过大小限制。"));
    }
    Ok(bytes)
}
fn collection(b: &Backend, c: &Config) -> Result<reqwest::Url> {
    let url = base(c)?;
    // Create only the configured directory, including intermediate user-specified segments.
    let mut parent = c.clone();
    parent.webdav_directory.clear();
    let mut current = base(&parent)?;
    for part in c
        .webdav_directory
        .trim_matches('/')
        .split('/')
        .filter(|p| !p.is_empty())
    {
        {
            let mut segments = current
                .path_segments_mut()
                .map_err(|_| invalid("目录地址无效。"))?;
            segments.pop_if_empty().push(part).push("");
        }
        let response = request(
            b,
            c,
            reqwest::Method::from_bytes(b"MKCOL").unwrap(),
            current.clone(),
        )?
        .send()
        .map_err(|_| invalid("云端目录创建失败。"))?;
        if !response.status().is_success() && response.status().as_u16() != 405 {
            return Err(invalid("WebDAV 目录无法创建，请检查权限。"));
        }
    }
    Ok(url)
}
pub fn list(b: &Backend) -> Result<Vec<RemoteBackup>> {
    let c = config(b)?;
    let url = base(&c)?;
    let response=send(request(b,&c,reqwest::Method::from_bytes(b"PROPFIND").unwrap(),url.clone())?.header("Depth","1").header("Content-Type","application/xml; charset=utf-8").body("<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\"><d:prop><d:resourcetype/></d:prop></d:propfind>"))?;
    let body = bounded(response, 4 * 1024 * 1024)?;
    let mut reader = quick_xml::Reader::from_reader(body.as_slice());
    let mut names = std::collections::BTreeSet::new();
    loop {
        match reader
            .read_event()
            .map_err(|_| invalid("WebDAV 列表 XML 无效。"))?
        {
            quick_xml::events::Event::Start(e) if e.local_name().as_ref() == "href" => {
                let text = reader
                    .read_text(e.name())
                    .map_err(|_| invalid("WebDAV 条目无效。"))?;
                let href = quick_xml::escape::unescape(&text)
                    .map_err(|_| invalid("云端路径编码无效。"))?;
                let remote = url.join(&href).map_err(|_| invalid("云端条目路径无效。"))?;
                if remote.origin() != url.origin()
                    || remote.query().is_some()
                    || remote.fragment().is_some()
                {
                    continue;
                }
                let Some(name) = remote.path().strip_prefix(url.path()) else {
                    continue;
                };
                if !name.contains('/') && file_name(name).is_ok() {
                    names.insert(name.to_owned());
                }
            }
            quick_xml::events::Event::DocType(_) => {
                return Err(invalid("WebDAV 响应不得包含外部实体。"))
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    Ok(names
        .into_iter()
        .rev()
        .map(|file_name| RemoteBackup { file_name })
        .collect())
}
pub fn test(b: &Backend) -> Result<bool> {
    let c = config(b)?;
    collection(b, &c)?;
    list(b)?;
    Ok(true)
}
fn begin(b: &Backend, phase: &str) -> Result<()> {
    let mut m = b.backups.lock().map_err(|_| invalid("备份服务不可用。"))?;
    if m.state.running {
        return Err(invalid("已有备份或云任务正在进行。"));
    }
    m.state.running = true;
    m.state.phase = phase.into();
    m.state.cloud_status = phase.into();
    Ok(())
}
fn finish(b: &Backend, result: &Result<impl Sized>, success: &str) {
    if let Ok(mut m) = b.backups.lock() {
        m.state.running = false;
        m.state.phase = "idle".into();
        match result {
            Ok(_) => {
                m.state.cloud_status = success.into();
                m.state.message = "云备份操作已完成".into();
                if success == "uploaded" {
                    m.state.last_upload_at = Some(now());
                }
            }
            Err(e) => {
                m.state.cloud_status = "failed".into();
                m.state.message = e.1.into();
            }
        }
    }
}
pub fn upload(b: &Backend, q: LocalRequest) -> Result<bool> {
    begin(b, "uploading")?;
    let result = (|| {
        let backup = application_backup::list(b)?
            .into_iter()
            .find(|v| v.backup_id == q.backup_id)
            .ok_or_else(missing)?;
        let path = Path::new(&backup.path);
        super::saves::files::ancestors(path)?;
        if fs::metadata(path)
            .map_err(|_| invalid("本地备份无法读取。"))?
            .len()
            > 2 * 1024 * 1024 * 1024
        {
            return Err(invalid("备份超过上传大小限制。"));
        }
        let bytes = fs::read(path).map_err(|_| invalid("本地备份无法读取。"))?;
        if application_backup::hash(&bytes) != backup.sha256 {
            return Err(invalid("本地备份已改变，未上传。"));
        }
        let c = config(b)?;
        let root = collection(b, &c)?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| invalid("备份名称无效。"))?;
        file_name(name)?;
        let destination = root.join(name).map_err(|_| invalid("云端目标无效。"))?;
        let temp = root
            .join(&format!(".{}.uploading", id()))
            .map_err(|_| invalid("云端临时路径无效。"))?;
        send(
            request(b, &c, reqwest::Method::PUT, temp.clone())?
                .header("Content-Type", "application/octet-stream")
                .body(bytes),
        )?;
        let moved = request(
            b,
            &c,
            reqwest::Method::from_bytes(b"MOVE").unwrap(),
            temp.clone(),
        )?
        .header("Destination", destination.as_str())
        .header("Overwrite", "F")
        .send()
        .map_err(|_| invalid("云端上传完成，但重命名未成功，可重试本地包。"))?;
        if !moved.status().is_success() {
            if moved.status().as_u16() != 412 {
                return Err(invalid("云端上传暂未完成，本地包已保留。"));
            }
            let existing = bounded(
                send(request(b, &c, reqwest::Method::GET, destination)?)?,
                2 * 1024 * 1024 * 1024,
            )?;
            if application_backup::hash(&existing) != backup.sha256 {
                return Err(invalid("云端已有同名不同内容，未覆盖。"));
            }
        }
        // The application never deletes completed cloud objects.
        b.database()?
            .put_setting(&format!("backup.uploaded.{}", backup.backup_id), &now())?;
        Ok(true)
    })();
    finish(b, &result, "uploaded");
    result
}
pub fn download(b: &Backend, q: RemoteRequest) -> Result<Backup> {
    file_name(&q.file_name)?;
    begin(b, "downloading")?;
    let result = (|| {
        let c = config(b)?;
        let url = base(&c)?
            .join(&q.file_name)
            .map_err(|_| invalid("下载地址无效。"))?;
        let bytes = bounded(
            send(request(b, &c, reqwest::Method::GET, url)?)?,
            2 * 1024 * 1024 * 1024,
        )?;
        let pack = application_backup::decode(&bytes, q.password.as_deref())?;
        let dir = b.data_directory.join("cloud-downloads");
        fs::create_dir_all(&dir).map_err(|_| invalid("下载目录无法创建。"))?;
        super::saves::files::ancestors(&dir)?;
        let path = dir.join(format!("GalgameManager-{}.gmbak", pack.manifest.backup_id));
        if path.exists() {
            if fs::read(&path).map_err(|_| invalid("已有下载无法读取。"))? != bytes {
                return Err(invalid("已有同名下载不同，未覆盖。"));
            }
        } else {
            application_backup::atomic_write(&path, &bytes)?;
        }
        let info = Backup {
            backup_id: pack.manifest.backup_id,
            path: path_text(&path)?,
            created_at: pack.manifest.created_at,
            reason: "cloud_download".into(),
            size_bytes: bytes.len() as u64,
            sha256: application_backup::hash(&bytes),
            categories: pack.manifest.categories,
            encrypted: backup_crypto::encrypted(&bytes),
        };
        let mut history: Vec<Backup> = b.database()?.setting("backup.history")?.unwrap_or_default();
        if !history.iter().any(|v| v.path == info.path) {
            history.insert(0, info.clone());
            history.truncate(10000);
            b.database()?.put_setting("backup.history", &history)?;
        }
        Ok(info)
    })();
    finish(b, &result, "downloaded");
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        time::Instant,
    };
    #[test]
    fn webdav_upload_list_and_download_use_real_local_http_and_never_overwrite() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let halted = stop.clone();
        let objects = Arc::new(Mutex::new(
            std::collections::BTreeMap::<String, Vec<u8>>::new(),
        ));
        let store = objects.clone();
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !halted.load(std::sync::atomic::Ordering::Relaxed) && Instant::now() < deadline {
                let (mut socket, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut data = Vec::new();
                let mut chunk = [0u8; 4096];
                let boundary = loop {
                    let n = socket.read(&mut chunk).unwrap();
                    if n == 0 {
                        break 0;
                    }
                    data.extend_from_slice(&chunk[..n]);
                    if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                if boundary == 0 {
                    continue;
                }
                let header = String::from_utf8(data[..boundary].to_vec()).unwrap();
                let mut line = header.lines().next().unwrap().split_whitespace();
                let method = line.next().unwrap();
                let path = line.next().unwrap().to_owned();
                let length = header
                    .lines()
                    .filter_map(|l| l.split_once(':'))
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                while data.len() < boundary + length {
                    let n = socket.read(&mut chunk).unwrap();
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&chunk[..n]);
                }
                let mut objects = store.lock().unwrap();
                let (status, body) = match method {
                    "PUT" => {
                        objects.insert(path, data[boundary..boundary + length].to_vec());
                        ("201 Created", Vec::new())
                    }
                    "MOVE" => {
                        assert!(header
                            .lines()
                            .any(|l| l.eq_ignore_ascii_case("overwrite: F")));
                        let destination = header
                            .lines()
                            .filter_map(|l| l.split_once(':'))
                            .find(|(k, _)| k.eq_ignore_ascii_case("destination"))
                            .map(|(_, v)| reqwest::Url::parse(v.trim()).unwrap().path().to_owned())
                            .unwrap();
                        if objects.contains_key(&destination) {
                            ("412 Precondition Failed", Vec::new())
                        } else {
                            let value = objects.remove(&path).unwrap();
                            objects.insert(destination, value);
                            ("201 Created", Vec::new())
                        }
                    }
                    "PROPFIND" => {
                        assert!(header.lines().any(|l| l.eq_ignore_ascii_case("depth: 1")));
                        let entries = objects
                            .keys()
                            .map(|n| format!("<x:response><x:href>{n}</x:href></x:response>"))
                            .collect::<String>();
                        (
                            "207 Multi-Status",
                            format!("<x:multistatus xmlns:x=\"DAV:\">{entries}</x:multistatus>")
                                .into_bytes(),
                        )
                    }
                    "GET" => objects
                        .get(&path)
                        .cloned()
                        .map(|v| ("200 OK", v))
                        .unwrap_or(("404 Not Found", Vec::new())),
                    "MKCOL" => ("201 Created", Vec::new()),
                    _ => ("405 Method Not Allowed", Vec::new()),
                };
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .unwrap();
                socket.write_all(&body).unwrap();
            }
        });
        let root = std::env::temp_dir().join(format!("gm-webdav-{}", id()));
        fs::create_dir_all(&root).unwrap();
        let b = Backend::open(root.join("data")).unwrap();
        let c = Config {
            webdav_url: format!("http://{address}/"),
            ..Config::default()
        };
        b.database()
            .unwrap()
            .put_setting(application_backup::KEY, &c)
            .unwrap();
        assert!(test(&b).unwrap());
        let original = application_backup::create(
            &b,
            application_backup::CreateRequest {
                categories: vec!["metadata".into()],
                directory: root.to_str().unwrap().into(),
                password: None,
            },
        )
        .unwrap();
        assert!(upload(
            &b,
            LocalRequest {
                backup_id: original.backup_id.clone()
            }
        )
        .unwrap());
        let files = list(&b).unwrap();
        assert_eq!(files.len(), 1);
        let restored = download(
            &b,
            RemoteRequest {
                file_name: files[0].file_name.clone(),
                password: None,
            },
        )
        .unwrap();
        assert_eq!(restored.sha256, original.sha256);
        assert!(upload(
            &b,
            LocalRequest {
                backup_id: original.backup_id
            }
        )
        .unwrap());
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        server.join().unwrap();
        drop(b);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cloud_paths_reject_cross_origin_queries_credentials_and_parent_directories() {
        let mut c = Config {
            webdav_url: "https://user:secret@example.test/dav".into(),
            ..Config::default()
        };
        assert!(base(&c).is_err());
        c.webdav_url = "https://example.test/dav?secret=value".into();
        assert!(base(&c).is_err());
        c.webdav_url = "https://example.test/dav".into();
        c.webdav_directory = "../other".into();
        assert!(base(&c).is_err());
        c.webdav_directory = "备份/游戏".into();
        assert!(base(&c).unwrap().as_str().contains("%"));
        assert!(file_name("../private.gmbak").is_err());
    }
}

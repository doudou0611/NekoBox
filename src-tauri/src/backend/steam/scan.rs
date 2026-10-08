use super::super::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read};

#[derive(Default, Deserialize)]
pub struct ScanRequest {
    pub steam_path: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct LocalGame {
    pub app_id: String,
    pub name: String,
    pub directory: String,
    pub library_path: String,
    pub size_bytes: u64,
    pub existing_game_id: Option<String>,
}
#[derive(Serialize)]
pub struct ScanResult {
    pub games: Vec<LocalGame>,
    pub library_count: usize,
    pub warnings: Vec<String>,
}
#[derive(Debug)]
enum Node {
    Text(String),
    Object(BTreeMap<String, Node>),
}
impl Node {
    fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Self::Object(v) => v.get(&key.to_ascii_lowercase()),
            _ => None,
        }
    }
    fn text(&self) -> Option<&str> {
        match self {
            Self::Text(v) => Some(v),
            _ => None,
        }
    }
    fn value(&self, key: &str) -> &str {
        self.get(key).and_then(Node::text).unwrap_or("")
    }
}
fn parse(input: &str) -> Result<Node> {
    let mut chars = input.trim_start_matches('\u{feff}').chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            for c in chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
            continue;
        }
        if c == '{' || c == '}' {
            tokens.push(c.to_string());
            continue;
        }
        let mut value = String::new();
        if c == '"' {
            let mut closed = false;
            while let Some(c) = chars.next() {
                if c == '"' {
                    closed = true;
                    break;
                }
                if c == '\\' && matches!(chars.peek(), Some('"' | '\\')) {
                    value.push(chars.next().unwrap());
                } else {
                    value.push(c);
                }
            }
            if !closed {
                return Err(invalid("Steam 清单引号未闭合。"));
            }
        } else {
            value.push(c);
            while chars
                .peek()
                .is_some_and(|c| !c.is_whitespace() && *c != '{' && *c != '}')
            {
                value.push(chars.next().unwrap());
            }
        }
        tokens.push(value);
    }
    fn object(tokens: &[String], index: &mut usize, depth: usize) -> Result<Node> {
        if depth > 24 {
            return Err(invalid("Steam 清单嵌套过深。"));
        }
        let mut values = BTreeMap::new();
        while let Some(key) = tokens.get(*index) {
            *index += 1;
            if key == "}" {
                if depth == 0 {
                    return Err(invalid("Steam 清单括号无效。"));
                }
                return Ok(Node::Object(values));
            }
            if key == "{" {
                return Err(invalid("Steam 清单缺少字段名。"));
            }
            let value = tokens
                .get(*index)
                .ok_or_else(|| invalid("Steam 清单字段缺少值。"))?;
            *index += 1;
            let value = if value == "{" {
                object(tokens, index, depth + 1)?
            } else if value == "}" {
                return Err(invalid("Steam 清单字段缺少值。"));
            } else {
                Node::Text(value.clone())
            };
            values.insert(key.to_ascii_lowercase(), value);
        }
        if depth != 0 {
            return Err(invalid("Steam 清单括号未闭合。"));
        }
        Ok(Node::Object(values))
    }
    object(&tokens, &mut 0, 0)
}
fn read(path: &Path) -> Result<Node> {
    let file = std::fs::File::open(path).map_err(|_| invalid("无法读取 Steam 清单。"))?;
    let mut text = String::new();
    file.take(1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|_| invalid("Steam 清单编码无效。"))?;
    if text.len() > 1024 * 1024 {
        return Err(invalid("Steam 清单超过大小限制。"));
    }
    parse(&text)
}
pub(crate) fn app_id(value: &str) -> Result<u32> {
    value
        .parse::<u32>()
        .ok()
        .filter(|id| *id > 0 && id.to_string() == value)
        .ok_or_else(|| invalid("Steam AppID 无效。"))
}
fn tool(name: &str, id: &str) -> bool {
    let name = name.to_lowercase();
    id == "228980"
        || name.contains("redistributable")
        || name.contains("dedicated server")
        || name.starts_with("steam linux runtime")
        || name == "proton"
        || [
            "proton experimental",
            "proton hotfix",
            "proton battleye",
            "proton easyanticheat",
        ]
        .iter()
        .any(|p| name.starts_with(p))
        || name
            .strip_prefix("proton ")
            .is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit()))
}
fn manifest(library: &Path, path: &Path) -> Result<Option<LocalGame>> {
    let root = read(path)?;
    let data = root
        .get("appstate")
        .ok_or_else(|| invalid("Steam 清单缺少 AppState。"))?;
    let id = data.value("appid");
    app_id(id)?;
    if path.file_name().and_then(|v| v.to_str()) != Some(format!("appmanifest_{id}.acf").as_str()) {
        return Err(invalid("Steam 清单 AppID 与文件名不一致。"));
    }
    let flags = data.value("stateflags").parse::<u32>().unwrap_or(0);
    if flags & 4 == 0 {
        return Ok(None);
    }
    let name = data.value("name").trim();
    if name.is_empty() || tool(name, id) {
        return Ok(None);
    }
    let folder = data.value("installdir");
    if folder.is_empty()
        || folder == "."
        || folder == ".."
        || folder.contains(['/', '\\', ':', '\0'])
    {
        return Err(invalid("Steam 安装目录字段无效。"));
    }
    let common = library
        .join("steamapps/common")
        .canonicalize()
        .map_err(|_| invalid("Steam 库目录不可访问。"))?;
    let dir = common
        .join(folder)
        .canonicalize()
        .map_err(|_| invalid("Steam 游戏安装目录不存在。"))?;
    if !dir.is_dir() || !dir.starts_with(&common) || dir == common {
        return Err(invalid("Steam 游戏目录超出库范围。"));
    }
    Ok(Some(LocalGame {
        app_id: id.into(),
        name: name.into(),
        directory: path_text(&dir)?,
        library_path: path_text(library)?,
        size_bytes: data.value("sizeondisk").parse().unwrap_or(0),
        existing_game_id: None,
    }))
}
pub(crate) fn verify(directory: &str, id: &str) -> Result<LocalGame> {
    app_id(id)?;
    let directory = absolute_directory(directory)?;
    let common = directory
        .parent()
        .ok_or_else(|| invalid("Steam 游戏目录无效。"))?;
    if common.file_name().and_then(|s| s.to_str()) != Some("common") {
        return Err(invalid("请选择 Steam 库中的已安装游戏。"));
    }
    let library = common
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| invalid("Steam 库目录无效。"))?;
    let game = manifest(
        library,
        &library.join(format!("steamapps/appmanifest_{id}.acf")),
    )?
    .ok_or_else(|| invalid("游戏尚未完整安装或已被卸载，请重新扫描。"))?;
    if Path::new(&game.directory) != directory {
        return Err(invalid("Steam AppID 与安装目录不一致。"));
    }
    Ok(game)
}
#[cfg(windows)]
fn registry_path() -> Option<PathBuf> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    for name in ["SteamPath", "InstallPath"] {
        let mut bytes = 0;
        let key = wide("Software\\Valve\\Steam");
        let name = wide(name);
        unsafe {
            if RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut bytes,
            ) != 0
                || bytes > 65536
            {
                continue;
            }
            let mut buffer = vec![0u16; bytes as usize / 2 + 1];
            if RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            ) == 0
            {
                let length = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
                let path = PathBuf::from(String::from_utf16_lossy(&buffer[..length]));
                if path.join("steamapps").is_dir() {
                    return Some(path);
                }
            }
        }
    }
    None
}
pub(crate) fn install_path() -> Result<PathBuf> {
    #[cfg(windows)]
    if let Some(path) = registry_path() {
        return Ok(path);
    }
    let mut paths = Vec::new();
    for key in ["STEAM_DIR", "STEAM_HOME", "STEAM_ROOT"] {
        if let Some(value) = std::env::var_os(key) {
            paths.push(PathBuf::from(value));
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.extend([
            home.join("Library/Application Support/Steam"),
            home.join(".local/share/Steam"),
            home.join(".steam/steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ]);
    }
    #[cfg(windows)]
    for key in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(value) = std::env::var_os(key) {
            paths.push(PathBuf::from(value).join("Steam"));
        }
    }
    paths
        .into_iter()
        .find(|p| p.join("steamapps").is_dir())
        .ok_or_else(|| {
            invalid("未找到 Steam 本地库，请选择 Steam 安装目录或 SteamLibrary 文件夹。")
        })
}
pub fn scan(b: &Backend, q: ScanRequest) -> Result<ScanResult> {
    let root = match q.steam_path {
        Some(path) => absolute_directory(&path)?,
        None => install_path()?,
    };
    let root = if root.file_name().is_some_and(|n| n == "steamapps") {
        root.parent().unwrap_or(&root).to_path_buf()
    } else {
        root
    };
    if !root.join("steamapps").is_dir() {
        return Err(invalid(
            "所选目录没有 steamapps，请选择 Steam 或 SteamLibrary 文件夹。",
        ));
    }
    let mut libraries = vec![root.clone()];
    let mut warnings = Vec::new();
    let file = root.join("steamapps/libraryfolders.vdf");
    if file.exists() {
        match read(&file) {
            Ok(data) => {
                if let Some(Node::Object(folders)) = data.get("libraryfolders") {
                    for (key, node) in folders {
                        if key.parse::<u32>().is_err() {
                            continue;
                        }
                        let path = node.text().unwrap_or_else(|| node.value("path"));
                        if !path.is_empty() {
                            libraries.push(PathBuf::from(path));
                        }
                    }
                }
            }
            Err(e) => warnings.push(format!("其他库目录读取失败，仅扫描当前库：{}", e.1)),
        }
    }
    let mut seen_libraries = std::collections::HashSet::new();
    let mut games = BTreeMap::new();
    for library in libraries {
        let Ok(library) = library.canonicalize() else {
            warnings.push(format!("库暂不可访问：{}", library.display()));
            continue;
        };
        if !seen_libraries.insert(library.clone()) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(library.join("steamapps")) else {
            warnings.push(format!("无法读取库：{}", library.display()));
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                continue;
            }
            match manifest(&library, &entry.path()) {
                Ok(Some(game)) => {
                    games.entry(game.app_id.clone()).or_insert(game);
                }
                Ok(None) => {}
                Err(e) => {
                    if warnings.len() < 100 {
                        warnings.push(format!("{name}：{}", e.1));
                    }
                }
            }
        }
    }
    let mut games = games.into_values().collect::<Vec<_>>();
    {
        let db = b.database()?;
        for game in &mut games {
            game.existing_game_id = db.steam_import_conflict(&game.app_id, &game.directory)?;
        }
    }
    games.sort_by_key(|g| g.name.to_lowercase());
    Ok(ScanResult {
        games,
        library_count: seen_libraries.len(),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vdf_handles_comments_escapes_and_rejects_truncated_objects() {
        let v =
            parse("// comment\n\"LibraryFolders\" { \"0\" { \"path\" \"C:\\\\Steam\" } }").unwrap();
        assert_eq!(
            v.get("libraryfolders")
                .unwrap()
                .get("0")
                .unwrap()
                .value("path"),
            "C:\\Steam"
        );
        for broken in ["\"a\" {", "\"a\"", "\"a\" \"unterminated", "}"] {
            assert!(parse(broken).is_err());
        }
    }
    #[test]
    fn scan_checks_manifest_state_paths_dedup_and_conflicts() {
        let root = std::env::temp_dir().join(format!("nekobox-steam-{}", id()));
        let library = root.join("Steam");
        std::fs::create_dir_all(library.join("steamapps/common/Game")).unwrap();
        let manifest_path = library.join("steamapps/appmanifest_123.acf");
        let write = |folder: &str, flags: u32| {
            std::fs::write(&manifest_path, format!("\"AppState\" {{ \"appid\" \"123\" \"name\" \"Game\" \"installdir\" \"{folder}\" \"StateFlags\" \"{flags}\" }}")).unwrap()
        };
        let b = Backend::open(root.join("data")).unwrap();
        write("Game", 4);
        let result = scan(
            &b,
            ScanRequest {
                steam_path: Some(library.to_string_lossy().into()),
            },
        )
        .unwrap();
        assert_eq!(result.games.len(), 1);
        assert!(verify(&result.games[0].directory, "123").is_ok());
        assert!(verify(&result.games[0].directory, "456").is_err());
        write("Game", 2);
        assert!(manifest(&library, &manifest_path).unwrap().is_none());
        write("../Game", 4);
        assert!(manifest(&library, &manifest_path).is_err());
        assert!(tool("Proton 9.0", "2805730"));
        assert!(!tool("Proton Adventure", "123"));
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
}

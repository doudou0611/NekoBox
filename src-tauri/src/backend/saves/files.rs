use super::types::{FileStamp, Manifest};
use crate::{
    backend::{Result, ServiceError},
    domain::ErrorCode,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

pub const MAX_TOTAL: u64 = 1024 * 1024 * 1024;
const MAX_FILE: u64 = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 10000;
pub fn conflict(message: &'static str) -> ServiceError {
    ServiceError(ErrorCode::Conflict, message)
}
fn io_error(_: impl std::fmt::Display) -> ServiceError {
    ServiceError(
        ErrorCode::PermissionDenied,
        "存档文件无法访问或写入，请关闭游戏并检查权限、空间和文件占用。",
    )
}
pub fn plain(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    #[cfg(windows)]
    let link = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let link = metadata.file_type().is_symlink();
    if link {
        return Err(conflict("存档路径包含链接或重解析点，操作已停止。"));
    }
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(conflict("存档路径包含不支持的特殊文件。"));
    }
    Ok(())
}
pub fn ancestors(path: &Path) -> Result<()> {
    for parent in path.ancestors() {
        plain(parent)?;
    }
    Ok(())
}
pub fn directory(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if !path.is_absolute() || value.contains('\0') {
        return Err(crate::backend::invalid("请选择绝对存档目录。"));
    }
    ancestors(path)?;
    let root = path.canonicalize().map_err(io_error)?;
    if !root.is_dir() || root.parent().is_none() {
        return Err(conflict("存档路径必须是独立目录，不能是磁盘根目录。"));
    }
    Ok(root)
}
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 4096
        && name.split('/').count() <= 16
        && name.split('/').all(|part| {
            let device = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.len() <= 240
                && !part.ends_with(['.', ' '])
                && !part
                    .chars()
                    .any(|c| c.is_control() || "<>:\\|?*".contains(c))
                && !matches!(
                    device.as_str(),
                    "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
                )
                && !((device.starts_with("COM") || device.starts_with("LPT"))
                    && device.chars().count() == 4
                    && matches!(device.chars().nth(3), Some('1'..='9' | '¹' | '²' | '³')))
        })
}
pub fn digest_reader(reader: &mut impl Read, limit: u64) -> Result<FileStamp> {
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let read = reader.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        size += read as u64;
        if size > limit {
            return Err(conflict("存档超过本轮安全大小限制，未执行操作。"));
        }
        hash.update(&buffer[..read]);
    }
    Ok(FileStamp {
        size_bytes: size,
        sha256: format!("{:x}", hash.finalize()),
    })
}
pub fn digest(path: &Path, limit: u64) -> Result<FileStamp> {
    ancestors(path)?;
    digest_reader(&mut File::open(path).map_err(io_error)?, limit)
}
pub fn manifest(root: &Path) -> Result<Manifest> {
    ancestors(root)?;
    let mut result = Manifest::new();
    let mut pending = vec![(root.to_owned(), 0)];
    let mut entries = 0;
    let mut total = 0;
    let mut names = std::collections::HashSet::new();
    while let Some((path, depth)) = pending.pop() {
        plain(&path)?;
        for entry in fs::read_dir(path).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let path = entry.path();
            entries += 1;
            if entries > MAX_ENTRIES || depth >= 16 {
                return Err(conflict("存档条目或目录深度超过安全限制。"));
            }
            plain(&path)?;
            if path.is_dir() {
                pending.push((path, depth + 1));
                continue;
            }
            let name = path
                .strip_prefix(root)
                .map_err(io_error)?
                .to_str()
                .ok_or_else(|| conflict("存档文件名编码不受支持。"))?
                .replace('\\', "/");
            if !valid_name(&name) || !names.insert(name.to_lowercase()) {
                return Err(conflict("存档包含不安全文件名或大小写冲突。"));
            }
            let stamp = digest(&path, MAX_FILE)?;
            total += stamp.size_bytes;
            if total > MAX_TOTAL {
                return Err(conflict("存档总大小超过 1 GiB，操作已停止。"));
            }
            result.insert(name, stamp);
        }
    }
    Ok(result)
}
pub fn write_archive(root: &Path, destination: &Path, expected: &Manifest) -> Result<()> {
    let output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(io_error)?;
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);
    for (name, stamp) in expected {
        let path = root.join(name);
        ancestors(&path)?;
        writer.start_file(name, options).map_err(io_error)?;
        let mut file = File::open(path).map_err(io_error)?;
        std::io::copy(&mut Read::by_ref(&mut file).take(MAX_FILE + 1), &mut writer)
            .map_err(io_error)?;
        if digest(&root.join(name), MAX_FILE)? != *stamp {
            return Err(conflict("备份时存档发生变化，请关闭游戏后重试。"));
        }
    }
    writer
        .finish()
        .map_err(io_error)?
        .sync_all()
        .map_err(io_error)?;
    if manifest(root)? != *expected {
        return Err(conflict("备份时存档发生变化，请关闭游戏后重试。"));
    }
    Ok(())
}
pub fn read_archive(path: &Path, expected_sha: &str, output: Option<&Path>) -> Result<Manifest> {
    let stamp = digest(path, MAX_TOTAL + 16 * 1024 * 1024)?;
    if stamp.sha256 != expected_sha {
        return Err(conflict("ZIP 的 SHA-256 校验失败，原存档未修改。"));
    }
    let mut archive = ZipArchive::new(File::open(path).map_err(io_error)?).map_err(io_error)?;
    if archive.len() > MAX_ENTRIES {
        return Err(conflict("快照文件数量超过安全限制。"));
    }
    let mut result = Manifest::new();
    let mut names = std::collections::HashSet::new();
    let mut total = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(io_error)?;
        let name = entry.name().to_owned();
        if entry.is_dir()
            || entry.is_symlink()
            || !valid_name(&name)
            || !names.insert(name.to_lowercase())
            || entry.size() > MAX_FILE
        {
            return Err(conflict("ZIP 包含不安全条目，原存档未修改。"));
        }
        let expected_size = entry.size();
        let stamp = if let Some(root) = output {
            let target = root.join(&name);
            fs::create_dir_all(target.parent().unwrap()).map_err(io_error)?;
            ancestors(target.parent().unwrap())?;
            if target.exists() {
                plain(&target)?;
            }
            let mut file = File::create(&target).map_err(io_error)?;
            let copied = std::io::copy(&mut entry.by_ref().take(MAX_FILE + 1), &mut file)
                .map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
            if copied > MAX_FILE {
                return Err(conflict("ZIP 解压大小超过安全限制。"));
            }
            digest(&target, MAX_FILE)?
        } else {
            digest_reader(&mut entry, MAX_FILE)?
        };
        if stamp.size_bytes != expected_size {
            return Err(conflict("ZIP 文件大小不一致，原存档未修改。"));
        }
        total += stamp.size_bytes;
        if total > MAX_TOTAL {
            return Err(conflict("ZIP 解压总大小超过安全限制。"));
        }
        result.insert(name, stamp);
    }
    Ok(result)
}
pub fn rename(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to).map_err(io_error)
}
pub fn make_directory(path: &Path) -> Result<()> {
    let mut missing = vec![];
    let mut parent = path;
    while !parent.exists() {
        missing.push(parent);
        parent = parent.parent().ok_or_else(|| conflict("备份目录无效。"))?;
    }
    ancestors(parent)?;
    for directory in missing.into_iter().rev() {
        fs::create_dir(directory).map_err(io_error)?;
        plain(directory)?;
    }
    ancestors(path)
}

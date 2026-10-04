use super::{invalid, permission, scanner, Result};
use image::{ImageFormat, ImageReader, Limits};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PIXELS: u64 = 32_000_000;
const MAX_EDGE: u32 = 16_384;

pub fn bounded_image(bytes: &[u8]) -> Result<image::DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| invalid("无法识别截图格式。"))?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif)
    ) {
        return Err(invalid("截图图片格式不受支持。"));
    }
    let (width, height) = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| invalid("无法读取截图尺寸。"))?
        .into_dimensions()
        .map_err(|_| invalid("截图已损坏或无法解码。"))?;
    if width > MAX_EDGE || height > MAX_EDGE || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(invalid("截图尺寸过大，已跳过。"));
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_EDGE);
    limits.max_image_height = Some(MAX_EDGE);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|_| invalid("截图已损坏或无法解码。"))
}
pub fn relative_name(value: &str) -> Option<&str> {
    let name = value.strip_prefix("screenshot-thumbnails/")?;
    let hash = name.strip_suffix(".png")?;
    (hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
    .then_some(name)
}
pub fn cached_path(data: &Path, relative: &str) -> Result<PathBuf> {
    let name = relative_name(relative).ok_or_else(|| invalid("缩略图缓存路径无效。"))?;
    let dir = data.join("screenshot-thumbnails");
    if fs::symlink_metadata(&dir).is_ok_and(|m| scanner::linked(&m)) {
        return Err(permission("缩略图目录不能是链接。"));
    }
    let path = dir.join(name);
    if fs::symlink_metadata(&path).is_ok_and(|m| scanner::linked(&m)) {
        return Err(permission("缩略图文件不能是链接。"));
    }
    Ok(path)
}
pub fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|_| permission("无法读取截图图片。"))?;
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() <= maximum)
    {
        return Err(invalid("截图文件过大或类型无效。"));
    }
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| permission("无法读取截图图片。"))?;
    if bytes.len() as u64 > maximum {
        return Err(invalid("截图文件已变化或超过限制。"));
    }
    Ok(bytes)
}
pub fn generate(data: &Path, source: &Path) -> Result<String> {
    let bytes = read_bounded(source, MAX_BYTES)?;
    // Version salt makes cache changes independent of existing application data.
    let mut hash = Sha256::new();
    hash.update(b"gm-thumbnail-v1-480x300");
    hash.update(&bytes);
    let relative = format!("screenshot-thumbnails/{:x}.png", hash.finalize());
    let target = cached_path(data, &relative)?;
    if let Ok(cached) = read_bounded(&target, 1024 * 1024) {
        if let Ok(image) = bounded_image(&cached) {
            if image.width() <= 480 && image.height() <= 300 {
                return Ok(relative);
            }
        }
    }
    let image = bounded_image(&bytes)?;
    let thumb = image.thumbnail(480, 300);
    let mut encoded = Cursor::new(Vec::new());
    thumb
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|_| invalid("无法生成截图缩略图。"))?;
    let parent = target.parent().expect("known cache subdirectory");
    fs::create_dir_all(parent).map_err(|_| permission("无法创建缩略图目录。"))?;
    let temp = parent.join(format!(".{}.tmp", super::id()));
    let write = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| permission("无法写入缩略图。"))?;
        file.write_all(encoded.get_ref())
            .and_then(|_| file.sync_all())
            .map_err(|_| permission("无法保存缩略图。"))?;
        if target.exists() {
            fs::remove_file(&target).map_err(|_| permission("无法更新缩略图缓存。"))?;
        }
        fs::rename(&temp, &target).map_err(|_| permission("无法保存缩略图缓存。"))?;
        Ok(relative)
    })();
    let _ = fs::remove_file(temp);
    write
}

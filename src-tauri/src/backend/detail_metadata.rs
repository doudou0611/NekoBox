//! Explicit local edits and per-game metadata freezing. No game files are changed.
use super::*;
use crate::domain::models::GameDetail;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, io::Read};
#[derive(Deserialize)]
pub struct EditRequest {
    pub game_id: String,
    pub changes: BTreeMap<String, Value>,
    pub expected: BTreeMap<String, Value>,
}
#[derive(Deserialize)]
pub struct LockRequest {
    pub game_id: String,
    pub locked: bool,
}
pub fn edit(b: &Backend, mut q: EditRequest) -> Result<GameDetail> {
    b.database()?.ensure_metadata_unlocked(&q.game_id)?;
    if let Some(Value::String(path)) = q.changes.get("cover_path") {
        if !path.is_empty() && !path.starts_with("covers/") {
            let path = Path::new(path);
            if !path.is_absolute() {
                return Err(invalid("封面请选择完整的图片路径。"));
            }
            let file =
                std::fs::File::open(path).map_err(|_| invalid("封面图片不存在或不可读取。"))?;
            let mut bytes = Vec::new();
            file.take(20 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| invalid("无法读取封面图片。"))?;
            if bytes.len() > 20 * 1024 * 1024 {
                return Err(invalid("封面图片不能超过 20 MiB。"));
            }
            // Read dimensions before decoding to reject decompression-sized images.
            let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
                .with_guessed_format()
                .map_err(|_| invalid("无法识别封面格式。"))?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(8192);
            limits.max_image_height = Some(8192);
            reader.limits(limits);
            let image = reader.decode().map_err(|_| {
                invalid("封面需要有效的 PNG、JPEG、WebP 或 GIF 图片，尺寸不超过 8192。")
            })?;
            let mut output = std::io::Cursor::new(Vec::new());
            image
                .write_to(&mut output, image::ImageFormat::Png)
                .map_err(|_| invalid("无法保存封面。"))?;
            let bytes = output.into_inner();
            use sha2::{Digest, Sha256};
            let relative = format!("covers/{:x}.png", Sha256::digest(&bytes));
            std::fs::create_dir_all(b.data_directory.join("covers"))
                .map_err(|_| permission("无法创建封面缓存目录。"))?;
            import_metadata::persist_cover(
                b,
                &b.data_directory.join(&relative),
                &bytes,
                &relative,
            )?;
            q.changes
                .insert("cover_path".into(), Value::String(relative));
        } else if !path.is_empty() {
            import_metadata::validate_cover(b, path)?;
        }
    }
    b.database()?.edit_metadata(&q)
}
pub fn lock(b: &Backend, q: LockRequest) -> Result<GameDetail> {
    let db = b.database()?;
    db.get_game(&q.game_id)?;
    if q.locked && !db.metadata_locked(&q.game_id)? {
        db.freeze_metadata_order(&q.game_id)?;
    }
    db.put_setting(&format!("metadata.locked.{}", q.game_id), &q.locked)?;
    db.get_game(&q.game_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn local_cover_is_validated_cached_and_guarded_by_the_lock() {
        let root = std::env::temp_dir().join(format!("nekobox-detail-{}", id()));
        let b = Backend::open(root.join("data")).unwrap();
        let game = b
            .database()
            .unwrap()
            .import_installation(
                "C:/fixture",
                "Fixture",
                None,
                crate::domain::protocol::InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        let image = image::RgbImage::from_pixel(16, 24, image::Rgb([125, 105, 150]));
        let path = root.join("owned-cover.png");
        image.save(&path).unwrap();
        let original = std::fs::read(&path).unwrap();
        let request = || EditRequest {
            game_id: game.clone(),
            changes: [("cover_path".into(), json!(path.to_str().unwrap()))]
                .into_iter()
                .collect(),
            expected: [("cover_path".into(), json!(null))].into_iter().collect(),
        };
        let value = edit(&b, request()).unwrap();
        let cache = value.summary.cover_url.unwrap();
        assert!(cache.starts_with("covers/"));
        assert!(b.data_directory.join(cache).is_file());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        lock(
            &b,
            LockRequest {
                game_id: game.clone(),
                locked: true,
            },
        )
        .unwrap();
        assert!(edit(&b, request()).is_err());
        lock(
            &b,
            LockRequest {
                game_id: game.clone(),
                locked: false,
            },
        )
        .unwrap();
        let invalid_path = root.join("not-an-image.png");
        std::fs::write(&invalid_path, b"fixture").unwrap();
        let mut invalid = request();
        invalid
            .changes
            .insert("cover_path".into(), json!(invalid_path.to_str().unwrap()));
        assert!(edit(&b, invalid).is_err());
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
}

use super::{absolute_directory, path_text, Backend, Result};
use crate::{
    backend::types::{
        NoteRequest, ReplaceGameTagsRequest, ScanScreenshotsRequest, ScreenshotQuery,
        UpdateScreenshotRequest,
    },
    domain::models::{Note, Screenshot, Tag},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_SCREENSHOTS: usize = 1000;

fn image_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp" | "gif")
    )
}
fn walk_images(root: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_owned(), 0u8)];
    while let Some((directory, depth)) = pending.pop() {
        if depth > 4 {
            continue;
        }
        for entry in fs::read_dir(&directory)
            .map_err(|_| super::permission("无法读取游戏目录中的截图文件。"))?
        {
            let entry = entry.map_err(|_| super::permission("无法读取游戏目录中的截图文件。"))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_| super::permission("无法读取截图文件信息。"))?;
            if super::scanner::linked(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                pending.push((path, depth + 1));
                continue;
            }
            if metadata.is_file() && image_extension(&path) {
                if found.len() >= MAX_SCREENSHOTS {
                    return Err(super::invalid("截图数量超过 1000 张，请缩小扫描目录。"));
                }
                found.push(path);
            }
        }
    }
    Ok(found)
}
fn public(screenshot: Screenshot) -> Screenshot {
    let id = screenshot.id.clone();
    Screenshot {
        image_url: format!("screenshot://localhost/{id}"),
        thumbnail_url: if let Some(name) =
            super::thumbnails::relative_name(&screenshot.thumbnail_url)
        {
            format!("screenshot://localhost/thumbnail/{id}/{name}")
        } else {
            format!("screenshot://localhost/{id}")
        },
        ..screenshot
    }
}
impl Backend {
    pub fn scan_screenshots(&self, request: ScanScreenshotsRequest) -> Result<u64> {
        let _guard = self
            .screenshot_lock
            .lock()
            .map_err(|_| super::invalid("截图服务需要重新启动。"))?;
        let installation = self.database()?.installation(&request.install_id)?;
        let root = absolute_directory(&installation.absolute_path)?;
        let files = walk_images(&root)?;
        let mut indexed = 0;
        for path in files {
            let canonical = path
                .canonicalize()
                .map_err(|_| super::permission("截图文件无法访问。"))?;
            if !canonical.starts_with(&root) {
                continue;
            }
            let metadata =
                fs::metadata(&canonical).map_err(|_| super::permission("截图文件无法访问。"))?;
            if metadata.len() > super::thumbnails::MAX_BYTES {
                continue;
            }
            let thumbnail = match super::thumbnails::generate(&self.data_directory, &canonical) {
                Ok(path) => path,
                Err(super::ServiceError(crate::domain::ErrorCode::InvalidRequest, _)) => continue,
                Err(e) => return Err(e),
            };
            let db = self.database()?;
            db.upsert_screenshot(
                &installation.game_id,
                &request.install_id,
                &path_text(&canonical)?,
                None,
            )?;
            db.set_screenshot_thumbnail(
                &installation.game_id,
                &path_text(&canonical)?,
                &thumbnail,
            )?;
            indexed += 1;
        }
        Ok(indexed)
    }
    pub fn list_screenshots(&self, request: ScreenshotQuery) -> Result<Vec<Screenshot>> {
        Ok(self
            .database()?
            .list_screenshots(&request.game_id, request.include_spoilers)?
            .into_iter()
            .map(public)
            .collect())
    }
    pub fn update_screenshot(&self, request: &UpdateScreenshotRequest) -> Result<Screenshot> {
        self.database()?.update_screenshot(request).map(public)
    }
    pub fn list_notes(&self, game_id: &str) -> Result<Vec<Note>> {
        self.database()?.list_notes(game_id)
    }
    pub fn save_note(&self, request: &NoteRequest) -> Result<Note> {
        self.database()?.save_note(request)
    }
    pub fn delete_note(&self, id: &str, confirmed: bool) -> Result<bool> {
        if !confirmed {
            return Err(super::invalid("删除笔记需要再次确认。"));
        }
        self.database()?.delete_note(id)
    }
    pub fn replace_game_tags(&self, request: &ReplaceGameTagsRequest) -> Result<Vec<Tag>> {
        self.database()?.replace_game_tags(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{id, BackendState};
    use crate::domain::protocol::InstallSource;
    use image::{ImageFormat, Rgba, RgbaImage};
    #[test]
    fn scan_generates_portable_thumbnails_and_protocol_checks_registered_ownership() {
        let root = std::env::temp_dir().join(format!("gm-thumbnails-{}", id()));
        fs::create_dir_all(root.join("game")).unwrap();
        let source = root.join("game/shot.png");
        RgbaImage::from_pixel(960, 600, Rgba([10, 120, 200, 255]))
            .save_with_format(&source, ImageFormat::Png)
            .unwrap();
        let original = fs::read(&source).unwrap();
        fs::write(root.join("game/broken.png"), b"invalid png").unwrap();
        RgbaImage::new(20000, 1)
            .save_with_format(root.join("game/oversized.png"), ImageFormat::Png)
            .unwrap();
        let data = root.join("data");
        let backend = Backend::open(data.clone()).unwrap();
        let game = backend
            .database()
            .unwrap()
            .import_installation(
                root.join("game").to_str().unwrap(),
                "作品",
                None,
                InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        let install = backend
            .database()
            .unwrap()
            .get_game(&game)
            .unwrap()
            .summary
            .installations[0]
            .id
            .clone();
        assert_eq!(
            backend
                .scan_screenshots(ScanScreenshotsRequest {
                    install_id: install.clone()
                })
                .unwrap(),
            1
        );
        let item = backend
            .list_screenshots(ScreenshotQuery {
                game_id: game.clone(),
                include_spoilers: true,
            })
            .unwrap()
            .remove(0);
        assert_ne!(item.image_url, item.thumbnail_url);
        assert_eq!(fs::read(&source).unwrap(), original);
        let relative = backend
            .database()
            .unwrap()
            .screenshot_thumbnail(&item.id)
            .unwrap()
            .unwrap();
        let thumb =
            super::super::thumbnails::cached_path(&backend.data_directory, &relative).unwrap();
        assert_eq!(image::open(&thumb).unwrap().width(), 480);
        assert_eq!(image::open(&thumb).unwrap().height(), 300);
        let old = fs::read(&thumb).unwrap();
        assert_eq!(
            backend
                .scan_screenshots(ScanScreenshotsRequest {
                    install_id: install.clone()
                })
                .unwrap(),
            1
        );
        assert_eq!(fs::read(&thumb).unwrap(), old);
        fs::write(&thumb, b"broken cache").unwrap();
        backend
            .scan_screenshots(ScanScreenshotsRequest {
                install_id: install.clone(),
            })
            .unwrap();
        assert_eq!(fs::read(&thumb).unwrap(), old);
        let state = BackendState(Ok(backend.clone()));
        let path = item
            .thumbnail_url
            .strip_prefix("screenshot://localhost")
            .unwrap();
        let response =
            crate::screenshot_protocol::response(Some(&state), path, "tauri://localhost");
        assert_eq!(response.status(), 200);
        assert_eq!(response.body(), &old);
        assert_eq!(
            crate::screenshot_protocol::response(
                Some(&state),
                &format!("/thumbnail/{}/{}.png", item.id, "f".repeat(64)),
                "tauri://localhost"
            )
            .status(),
            400
        );
        assert_eq!(
            crate::screenshot_protocol::response(
                Some(&state),
                &format!(
                    "/thumbnail/{}/{}",
                    id(),
                    thumb.file_name().unwrap().to_str().unwrap()
                ),
                "tauri://localhost"
            )
            .status(),
            404
        );
        let edited = backend
            .update_screenshot(&UpdateScreenshotRequest {
                game_id: game.clone(),
                screenshot_id: item.id.clone(),
                title: "  结局  ".into(),
                captured_at: Some("2026-10-01T12:00:00+08:00".into()),
                is_spoiler: true,
            })
            .unwrap();
        assert_eq!(edited.title.as_deref(), Some("结局"));
        assert_eq!(
            edited.captured_at.as_deref(),
            Some("2026-10-01T04:00:00.000Z")
        );
        assert!(backend
            .list_screenshots(ScreenshotQuery {
                game_id: game.clone(),
                include_spoilers: false
            })
            .unwrap()
            .is_empty());
        assert!(backend
            .update_screenshot(&UpdateScreenshotRequest {
                game_id: id(),
                screenshot_id: item.id.clone(),
                title: "越界".into(),
                captured_at: None,
                is_spoiler: false
            })
            .is_err());
        assert!(backend
            .update_screenshot(&UpdateScreenshotRequest {
                game_id: game.clone(),
                screenshot_id: item.id.clone(),
                title: "不应写入".into(),
                captured_at: Some("bad-date".into()),
                is_spoiler: false
            })
            .is_err());
        #[cfg(unix)]
        {
            fs::remove_file(&thumb).unwrap();
            std::os::unix::fs::symlink(&source, &thumb).unwrap();
            assert_eq!(
                crate::screenshot_protocol::response(Some(&state), path, "tauri://localhost")
                    .status(),
                403
            );
            fs::remove_file(&thumb).unwrap();
        }
        RgbaImage::from_pixel(960, 600, Rgba([200, 10, 20, 255]))
            .save_with_format(&source, ImageFormat::Png)
            .unwrap();
        backend
            .scan_screenshots(ScanScreenshotsRequest {
                install_id: install,
            })
            .unwrap();
        let new_relative = backend
            .database()
            .unwrap()
            .screenshot_thumbnail(&item.id)
            .unwrap()
            .unwrap();
        assert_ne!(relative, new_relative);
        drop(state);
        drop(backend);
        let moved = root.join("moved-data");
        fs::rename(data, &moved).unwrap();
        let reopened = BackendState(Backend::open(moved));
        let moved_path = format!(
            "/thumbnail/{}/{}",
            item.id,
            super::super::thumbnails::relative_name(&new_relative).unwrap()
        );
        assert_eq!(
            crate::screenshot_protocol::response(Some(&reopened), &moved_path, "tauri://localhost")
                .status(),
            200
        );
        assert_eq!(
            reopened
                .0
                .as_ref()
                .unwrap()
                .database()
                .unwrap()
                .list_screenshots(&game, true)
                .unwrap()[0]
                .title
                .as_deref(),
            Some("结局")
        );
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
}

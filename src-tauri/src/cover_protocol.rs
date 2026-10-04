//! Serve only immutable cover-cache filenames, never frontend-selected filesystem paths.
use std::{io::Read, path::Path};
use tauri::http::{header, Response, StatusCode};

const LIMIT: u64 = 8 * 1024 * 1024;
fn mime(filename: &str) -> Option<&'static str> {
    let (hash, extension) = filename.rsplit_once('.')?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    match extension {
        "jpg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}
pub fn response(directory: Option<&Path>, uri_path: &str, origin: &str) -> Response<Vec<u8>> {
    let make = |status, content_type: &str, body: Vec<u8>| {
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin)
            .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
            .header(
                header::CACHE_CONTROL,
                if status == StatusCode::OK {
                    "private, max-age=31536000, immutable"
                } else {
                    "no-store"
                },
            )
            .body(body)
            .expect("static cover response headers")
    };
    let Some(filename) = uri_path.strip_prefix('/') else {
        return make(StatusCode::BAD_REQUEST, "text/plain", Vec::new());
    };
    let Some(content_type) = mime(filename) else {
        return make(StatusCode::BAD_REQUEST, "text/plain", Vec::new());
    };
    let Some(directory) = directory else {
        return make(StatusCode::SERVICE_UNAVAILABLE, "text/plain", Vec::new());
    };
    let covers = directory.join("covers");
    // Reject cache symlinks/reparse links that escape the program's cache directory.
    let path = covers.join(filename);
    let resolved = covers
        .canonicalize()
        .and_then(|root| path.canonicalize().map(|path| (root, path)));
    let Ok((root, path)) = resolved else {
        return make(StatusCode::NOT_FOUND, "text/plain", Vec::new());
    };
    if path.parent() != Some(root.as_path()) {
        return make(StatusCode::FORBIDDEN, "text/plain", Vec::new());
    }
    let Ok(file) = std::fs::File::open(path) else {
        return make(StatusCode::NOT_FOUND, "text/plain", Vec::new());
    };
    let mut bytes = Vec::new();
    if file.take(LIMIT + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > LIMIT {
        return make(StatusCode::INTERNAL_SERVER_ERROR, "text/plain", Vec::new());
    }
    make(StatusCode::OK, content_type, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serves_cached_bytes_offline_and_rejects_arbitrary_paths() {
        let directory = std::env::var_os("GALGAME_COVER_TEST_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("gm-protocol-{}", crate::backend::id()));
        std::fs::create_dir_all(directory.join("covers")).unwrap();
        let filename = format!("{}.jpg", "a".repeat(64));
        let bytes = b"\xff\xd8\xff\xe0local cached picture";
        std::fs::write(directory.join("covers").join(&filename), bytes).unwrap();
        let result = response(
            Some(&directory),
            &format!("/{filename}"),
            "http://tauri.localhost",
        );
        assert_eq!(result.status(), StatusCode::OK);
        assert_eq!(result.headers()[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(result.body(), bytes);
        for path in [
            "/../galgame-manager.sqlite3",
            "/C:/Windows/file.jpg",
            "/%2e%2e%2fsecret.jpg",
            "/nested/file.jpg",
            "/a.jpg",
            "/",
        ] {
            assert_eq!(
                response(Some(&directory), path, "http://tauri.localhost").status(),
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(
            response(
                Some(&directory),
                &format!("/{}.jpg", "b".repeat(64)),
                "http://tauri.localhost"
            )
            .status(),
            StatusCode::NOT_FOUND
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn only_known_image_types_are_served() {
        for (extension, expected) in [
            ("jpg", "image/jpeg"),
            ("png", "image/png"),
            ("webp", "image/webp"),
            ("gif", "image/gif"),
        ] {
            assert_eq!(
                mime(&format!("{}.{extension}", "a".repeat(64))),
                Some(expected)
            );
        }
        assert!(mime(&format!("{}.svg", "a".repeat(64))).is_none());
        assert!(mime(&format!("{}.jpg", "z".repeat(64))).is_none());
    }
}

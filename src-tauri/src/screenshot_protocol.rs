use crate::backend::BackendState;
use tauri::http::{header, Response, StatusCode};

pub fn response(state: Option<&BackendState>, path: &str, origin: &str) -> Response<Vec<u8>> {
    let make = |status: StatusCode, body: Vec<u8>| {
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin)
            .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
            .body(body)
            .expect("static screenshot response headers")
    };
    let raw_path = path.strip_prefix('/').unwrap_or("");
    let thumbnail = raw_path.strip_prefix("thumbnail/");
    let (id, requested_name) = match thumbnail {
        Some(value) => match value.split_once('/') {
            Some(parts) => (parts.0, Some(parts.1)),
            None => return make(StatusCode::BAD_REQUEST, Vec::new()),
        },
        None => (raw_path, None),
    };
    if uuid::Uuid::parse_str(id).is_err() {
        return make(StatusCode::BAD_REQUEST, Vec::new());
    }
    let Some(state) = state else {
        return make(StatusCode::SERVICE_UNAVAILABLE, Vec::new());
    };
    let Ok(backend) = state.0.as_ref() else {
        return make(StatusCode::SERVICE_UNAVAILABLE, Vec::new());
    };
    if let Some(name) = requested_name {
        let Ok(Some(relative)) = backend
            .database()
            .and_then(|db| db.screenshot_thumbnail(id))
        else {
            return make(StatusCode::NOT_FOUND, Vec::new());
        };
        if crate::backend::thumbnails::relative_name(&relative) != Some(name) {
            return make(StatusCode::BAD_REQUEST, Vec::new());
        }
        let Ok(path) = crate::backend::thumbnails::cached_path(&backend.data_directory, &relative)
        else {
            return make(StatusCode::FORBIDDEN, Vec::new());
        };
        let Ok(body) = crate::backend::thumbnails::read_bounded(&path, 1024 * 1024) else {
            return make(StatusCode::NOT_FOUND, Vec::new());
        };
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin)
            .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
            .header(
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            )
            .body(body)
            .expect("static thumbnail headers");
    }
    let Ok(Some((raw, game_id))) = backend.database().and_then(|db| db.screenshot_path(id)) else {
        return make(StatusCode::NOT_FOUND, Vec::new());
    };
    let Ok(detail) = backend.database().and_then(|db| db.get_game(&game_id)) else {
        return make(StatusCode::NOT_FOUND, Vec::new());
    };
    if std::path::Path::new(&raw)
        .ancestors()
        .any(|p| std::fs::symlink_metadata(p).is_ok_and(|m| crate::backend::scanner::linked(&m)))
    {
        return make(StatusCode::FORBIDDEN, Vec::new());
    }
    let Ok(path) = std::path::Path::new(&raw).canonicalize() else {
        return make(StatusCode::NOT_FOUND, Vec::new());
    };
    let inside_install = detail
        .summary
        .installations
        .iter()
        .filter_map(|i| std::path::Path::new(&i.absolute_path).canonicalize().ok())
        .any(|root| path.starts_with(root));
    let inside_restored = crate::backend::screenshot_actions::restored_original(backend, &path);
    if !inside_install && !inside_restored {
        return make(StatusCode::FORBIDDEN, Vec::new());
    }
    let content_type = match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => return make(StatusCode::BAD_REQUEST, Vec::new()),
    };
    let Ok(metadata) = std::fs::metadata(&path) else {
        return make(StatusCode::NOT_FOUND, Vec::new());
    };
    if metadata.len() > 64 * 1024 * 1024 {
        return make(StatusCode::PAYLOAD_TOO_LARGE, Vec::new());
    }
    let Ok(body) = crate::backend::thumbnails::read_bounded(&path, 64 * 1024 * 1024) else {
        return make(StatusCode::NOT_FOUND, Vec::new());
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CACHE_CONTROL, "private, max-age=60")
        .body(body)
        .expect("static screenshot response headers")
}

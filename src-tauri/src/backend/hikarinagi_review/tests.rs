use super::*;
use crate::domain::protocol::InstallSource;
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
};
struct Fixture {
    backend: Backend,
    root: PathBuf,
    game: String,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("gm-review-{}", super::super::id()));
        let backend = Backend::open(root.join("data")).unwrap();
        let game = backend
            .database()
            .unwrap()
            .import_installation(
                "/fixture",
                "评论夹具",
                None,
                InstallSource::Manual,
                &[],
                "review",
            )
            .unwrap();
        let f = Self {
            backend,
            root,
            game,
        };
        f.bind("897");
        f.backend
            .database()
            .unwrap()
            .put_setting(
                "hikarinagi.profile",
                &json!({"id":7,"username":"fixture","nickname":"夹具","avatar_url":null}),
            )
            .unwrap();
        f
    }
    fn bind(&self, id: &str) {
        self.backend
            .database()
            .unwrap()
            .apply_remote_fields(
                &self.game,
                "hikarinagi",
                id,
                &[("title_zh".into(), "评论夹具".into())],
                &[],
                &super::super::now(),
                true,
            )
            .unwrap();
    }
    fn request(&self) -> SubmitRequest {
        SubmitRequest {
            game_id: self.game.clone(),
            account_id: 7,
            remote_id: "897".into(),
            score: 8,
            comment: " 新评论\n第二行 ".into(),
            expected_updated_at: Some("2026-10-03T01:00:00Z".into()),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn token() -> Result<Option<String>> {
    Ok(Some("account-fixture".into()))
}
fn old() -> Value {
    json!({"id":1,"rate":6,"rate_content":"原短评","updated_at":"2026-10-03T01:00:00Z","status":"ON_HOLD","status_private":true,"is_spoiler":true,"rate_scenario":9.5,"rate_direction":null,"rate_music":7,"rate_visual":9,"rate_character":8,"rate_system":6,"time_to_finish_minutes":100,"like_count":12})
}
#[test]
fn unsigned_invalid_and_switched_accounts_never_write() {
    let f = Fixture::new();
    assert_eq!(
        submit_with(
            &f.backend,
            f.request(),
            || Ok(None),
            |_, _| panic!("no anonymous read"),
            |_, _, _| panic!("no anonymous write")
        )
        .unwrap_err()
        .0,
        ErrorCode::PermissionDenied
    );
    for score in [0, 11] {
        let mut request = f.request();
        request.score = score;
        assert_eq!(
            submit_with(
                &f.backend,
                request,
                || panic!("invalid request must not read credentials"),
                |_, _| panic!(),
                |_, _, _| panic!()
            )
            .unwrap_err()
            .0,
            ErrorCode::InvalidRequest
        );
    }
    let mut request = f.request();
    request.account_id = 8;
    assert_eq!(
        submit_with(
            &f.backend,
            request,
            token,
            |_, _| panic!(),
            |_, _, _| panic!()
        )
        .unwrap_err()
        .0,
        ErrorCode::Conflict
    );
    let mut request = f.request();
    request.comment = "字".repeat(5001);
    assert!(submit_with(
        &f.backend,
        request,
        token,
        |_, _| panic!(),
        |_, _, _| panic!()
    )
    .is_err());
    f.backend
        .database()
        .unwrap()
        .delete_setting("hikarinagi.profile")
        .unwrap();
    assert_eq!(
        submit_with(
            &f.backend,
            f.request(),
            token,
            |_, _| panic!(),
            |_, _, _| panic!()
        )
        .unwrap_err()
        .0,
        ErrorCode::PermissionDenied
    );
}
#[test]
fn concurrent_submission_is_rejected_before_network_or_credentials() {
    let f = Fixture::new();
    let _guard = SUBMISSION.lock().unwrap();
    assert_eq!(
        submit(&f.backend, f.request()).unwrap_err().0,
        ErrorCode::Conflict
    );
}
#[test]
fn record_conflicts_and_rebindings_never_write() {
    let f = Fixture::new();
    let mut request = f.request();
    request.expected_updated_at = None;
    assert_eq!(
        submit_with(
            &f.backend,
            request,
            token,
            |_, _| Ok(Some(old())),
            |_, _, _| panic!()
        )
        .unwrap_err()
        .0,
        ErrorCode::Conflict
    );
    assert_eq!(
        submit_with(
            &f.backend,
            f.request(),
            token,
            |_, _| {
                f.bind("898");
                Ok(Some(old()))
            },
            |_, _, _| panic!()
        )
        .unwrap_err()
        .0,
        ErrorCode::Cancelled
    );
    assert_eq!(
        submit_with(
            &f.backend,
            f.request(),
            token,
            |_, _| panic!(),
            |_, _, _| panic!()
        )
        .unwrap_err()
        .0,
        ErrorCode::Cancelled
    );
}
#[test]
fn submissions_preserve_other_remote_fields_and_public_snapshot() {
    let f = Fixture::new();
    let wall = json!({"remote_id":"897","source_url":"https://www.hikarinagi.org/galgames/897/rates","average":8.2,"rated_count":9,"distribution":[],"status_counts":{"completed":0,"going":0,"on_hold":0,"dropped":0},"keywords":[],"fetched_at":"2026-10-03T01:00:00Z","cached":false,"stale":false,"message":null});
    f.backend
        .database()
        .unwrap()
        .save_metadata_cache(
            "hikarinagi",
            "rates-wall:897",
            Some(&wall.to_string()),
            "success",
            None,
            "2026-10-03T01:00:00Z",
        )
        .unwrap();
    let submitted = submit_with(
        &f.backend,
        f.request(),
        token,
        |_, _| Ok(Some(old())),
        |id, token, data| {
            assert_eq!(id, "897");
            assert_eq!(token, "account-fixture");
            for field in [
                "status",
                "status_private",
                "is_spoiler",
                "rate_scenario",
                "rate_direction",
                "rate_music",
                "rate_visual",
                "rate_character",
                "rate_system",
                "time_to_finish_minutes",
            ] {
                assert_eq!(data[field], old()[field]);
            }
            assert_eq!(data["rate"], 8);
            assert_eq!(data["rate_content"], "新评论\n第二行");
            assert!(data.get("id").is_none());
            assert!(data.get("like_count").is_none());
            let mut response = old();
            response["rate"] = data["rate"].clone();
            response["rate_content"] = data["rate_content"].clone();
            Ok(response)
        },
    )
    .unwrap();
    assert_eq!(submitted.review.score, Some(8.0));
    let cached = f
        .backend
        .database()
        .unwrap()
        .metadata_cache_response("hikarinagi", "rates-wall:897")
        .unwrap()
        .unwrap();
    let cached: Value = serde_json::from_str(&cached.0).unwrap();
    assert_eq!(cached["stale"], false);
    assert_eq!(cached["fetched_at"], "2026-10-03T01:00:00Z");
    assert!(!cached.to_string().contains("新评论"));
    assert!(submitted.cache_warning.is_none());
}
#[test]
fn new_review_and_malformed_receipts_are_distinguished() {
    let f = Fixture::new();
    let found = get_with(
        &f.backend,
        GetRequest {
            game_id: f.game.clone(),
            account_id: 7,
        },
        token,
        |_, _| Ok(None),
    )
    .unwrap();
    assert_eq!(found.remote_id, "897");
    assert!(found.review.is_none());
    let mut request = f.request();
    request.expected_updated_at = None;
    let result = submit_with(
        &f.backend,
        request,
        token,
        |_, _| Ok(None),
        |_, _, data| {
            assert_eq!(data.as_object().unwrap().len(), 2);
            let mut result = old();
            result["rate"] = json!(8);
            Ok(result)
        },
    )
    .unwrap();
    assert_eq!(result.review.score, Some(8.0));
    assert_eq!(
        submit_with(
            &f.backend,
            f.request(),
            token,
            |_, _| Ok(Some(old())),
            |_, _, _| Ok(json!({"success":true}))
        )
        .unwrap_err()
        .0,
        ErrorCode::InvalidResponse
    );
    assert_eq!(
        submit_with(
            &f.backend,
            f.request(),
            token,
            |_, _| Ok(Some(old())),
            |_, _, _| Err(ServiceError(ErrorCode::NetworkUnavailable, "lost response"))
        )
        .unwrap_err()
        .0,
        ErrorCode::NetworkUnavailable
    );
}
#[test]
fn official_path_and_bearer_put_are_exercised_with_an_isolated_http_server() {
    let official = reqwest::Url::parse(&url("897")).unwrap();
    assert_eq!(official.host_str(), Some("api.hikarinagi.org"));
    assert_eq!(official.path(), "/api/v3/open/user/me/rates/galgames/897");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        for method in ["GET", "PUT"] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let count = stream.read(&mut chunk).unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&chunk[..count]);
                if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|s| s.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.starts_with(&format!(
                "{method} /api/v3/open/user/me/rates/galgames/897 HTTP/1.1"
            )));
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer account-fixture\r\n"));
            let mut result = old();
            if method == "PUT" {
                let data: Value =
                    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                assert_eq!(data["rate"], 8);
                result["rate"] = data["rate"].clone();
                result["rate_content"] = data["rate_content"].clone();
            }
            let body = json!({"success":true,"data":result}).to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    let f = Fixture::new();
    let client = hikarinagi_account::client().unwrap();
    let endpoint = format!("http://{address}{}", official.path());
    submit_with(
        &f.backend,
        f.request(),
        token,
        |_, token| read_at(&client, &endpoint, token),
        |_, token, data| write_at(&client, &endpoint, token, data),
    )
    .unwrap();
    worker.join().unwrap();
}

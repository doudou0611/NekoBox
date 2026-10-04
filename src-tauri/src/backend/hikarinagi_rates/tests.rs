use super::*;
use crate::domain::protocol::InstallSource;
use serde_json::json;
use std::{cell::Cell, fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    backend: Backend,
    game_id: String,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("gm-rates-{}", super::super::id()));
        let backend = Backend::open(root.join("data")).unwrap();
        let game_id = backend
            .database()
            .unwrap()
            .import_installation(
                root.join("game").to_str().unwrap(),
                "夹具作品",
                None,
                InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        Self {
            root,
            backend,
            game_id,
        }
    }
    fn bind(&self, id: &str) {
        self.backend
            .database()
            .unwrap()
            .apply_remote_fields(
                &self.game_id,
                "hikarinagi",
                id,
                &[("title_zh".into(), "夹具作品".into())],
                &[],
                &now(),
                true,
            )
            .unwrap();
    }
    fn request(&self, refresh: bool) -> Request {
        Request {
            game_id: self.game_id.clone(),
            refresh,
        }
    }
    fn cache(&self, wall: &Wall) {
        self.backend
            .database()
            .unwrap()
            .save_metadata_cache(
                "hikarinagi",
                &format!("rates-wall:{}", wall.remote_id),
                Some(&serde_json::to_string(wall).unwrap()),
                "success",
                None,
                &wall.fetched_at,
            )
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn values() -> (Value, Value) {
    (
        json!({"success":true,"data":{"average":8.2,"rated_count":9,"distribution":[{"score":6,"count":2},{"score":8,"count":3},{"score":9,"count":2},{"score":10,"count":2}],"status_counts":{"going":1,"completed":18,"on_hold":1,"dropped":2}}}),
        json!({"success":true,"data":{"keywords":[{"word":"CG","count":4},{"word":"妹妹","count":3}]}}),
    )
}
fn wall(id: &str) -> Wall {
    let (stats, keywords) = values();
    parse(id, &stats, &keywords).unwrap()
}
#[test]
fn parses_public_statistics_and_rejects_invalid_numeric_data() {
    let (mut stats, keywords) = values();
    let parsed = parse("897", &stats, &keywords).unwrap();
    assert_eq!(parsed.average, Some(8.2));
    assert_eq!(parsed.rated_count, 9);
    assert_eq!(parsed.status_counts.completed, 18);
    assert_eq!(parsed.keywords[0].word, "CG");
    assert!(!parsed.cached);
    for average in [json!(-1), json!(11), json!("8.2"), Value::Null] {
        stats["data"]["average"] = average;
        assert!(parse("897", &stats, &keywords).is_err());
    }
    let (mut stats, keywords) = values();
    stats["data"]["distribution"][0]["count"] = json!(-1);
    assert!(parse("897", &stats, &keywords).is_err());
    let (mut stats, keywords) = values();
    stats["data"]["distribution"][0]["score"] = json!(10);
    assert!(parse("897", &stats, &keywords).is_err());
    let (mut stats, keywords) = values();
    stats["data"]["rated_count"] = json!(10);
    assert!(parse("897", &stats, &keywords).is_err());
}
#[test]
fn no_binding_never_requests_network_and_null_average_is_not_zero() {
    let f = Fixture::new();
    assert!(
        get_with(&f.backend, f.request(false), |_| panic!("unbound request"))
            .unwrap()
            .wall
            .is_none()
    );
    let mut stats = json!({"success":true,"data":{"average":null,"rated_count":0,"distribution":[],"status_counts":{"going":0,"completed":0,"on_hold":0,"dropped":0}}});
    let parsed = parse(
        "897",
        &stats,
        &json!({"success":true,"data":{"keywords":[]}}),
    )
    .unwrap();
    assert!(parsed.average.is_none());
    assert!(parsed.keywords.is_empty());
    // A provider's zero sentinel for no votes must not become a zero-point review.
    stats["data"]["average"] = json!(0);
    assert!(parse(
        "897",
        &stats,
        &json!({"success":true,"data":{"keywords":[]}})
    )
    .unwrap()
    .average
    .is_none());
}
#[test]
fn cache_is_reused_and_persists_after_reopen_without_network() {
    let mut f = Fixture::new();
    f.bind("897");
    let requests = Cell::new(0);
    let response = get_with(&f.backend, f.request(false), |id| {
        requests.set(requests.get() + 1);
        Ok(wall(id))
    })
    .unwrap()
    .wall
    .unwrap();
    assert!(!response.cached);
    assert_eq!(requests.get(), 1);
    // Release the original instance's portable-directory lock before reopening.
    f.backend = Backend::open(f.root.join("other-data")).unwrap();
    let reopened = Backend::open(f.root.join("data")).unwrap();
    let cached = get_with(&reopened, f.request(false), |_| {
        panic!("fresh cache requested network")
    })
    .unwrap()
    .wall
    .unwrap();
    assert!(cached.cached && !cached.stale);
    assert_eq!(cached.fetched_at, response.fetched_at);
}
#[test]
fn old_snapshot_is_local_until_explicit_refresh_and_failed_refresh_preserves_timestamp() {
    let f = Fixture::new();
    f.bind("897");
    let mut old = wall("897");
    old.fetched_at = "2020-01-01T00:00:00Z".into();
    f.cache(&old);
    let local = get_with(&f.backend, f.request(false), |_| {
        panic!("old snapshot must stay local")
    })
    .unwrap()
    .wall
    .unwrap();
    assert!(local.cached && !local.stale);
    assert_eq!(local.fetched_at, old.fetched_at);
    {
        let cached = get_with(&f.backend, f.request(true), |_| {
            Err(ServiceError(ErrorCode::NetworkUnavailable, "网络不可用。"))
        })
        .unwrap()
        .wall
        .unwrap();
        assert!(cached.cached && cached.stale);
        assert_eq!(cached.fetched_at, old.fetched_at);
        assert!(cached.message.unwrap().contains("上次获取"));
    }
    let fresh = get_with(&f.backend, f.request(true), |id| Ok(wall(id)))
        .unwrap()
        .wall
        .unwrap();
    assert!(!fresh.cached && !fresh.stale);
    assert_ne!(fresh.fetched_at, old.fetched_at);
}
#[test]
fn rebind_uses_new_remote_id_and_discards_inflight_old_results() {
    let f = Fixture::new();
    f.bind("897");
    f.cache(&wall("897"));
    f.bind("123");
    let fresh = get_with(&f.backend, f.request(false), |id| {
        assert_eq!(id, "123");
        Ok(wall(id))
    })
    .unwrap()
    .wall
    .unwrap();
    assert_eq!(fresh.remote_id, "123");
    let result = get_with(&f.backend, f.request(true), |id| {
        f.bind("456");
        Ok(wall(id))
    });
    assert_eq!(result.unwrap_err().0, ErrorCode::Cancelled);
}
#[test]
fn failure_without_cache_does_not_create_a_fake_score() {
    let f = Fixture::new();
    f.bind("897");
    let result = get_with(&f.backend, f.request(false), |_| {
        Err(ServiceError(ErrorCode::RateLimited, "限流。"))
    });
    assert_eq!(result.unwrap_err().0, ErrorCode::RateLimited);
    assert!(f
        .backend
        .database()
        .unwrap()
        .metadata_cache_response("hikarinagi", "rates-wall:897")
        .unwrap()
        .is_none());
}
#[test]
fn legacy_stale_snapshot_does_not_trigger_network() {
    let f = Fixture::new();
    f.bind("897");
    let mut original = wall("897");
    original.stale = true;
    original.fetched_at = "2020-01-01T00:00:00Z".into();
    f.cache(&original);
    let response = get_with(&f.backend, f.request(false), |_| {
        panic!("stale snapshot requested network")
    })
    .unwrap()
    .wall
    .unwrap();
    assert!(response.cached);
    assert_eq!(response.fetched_at, original.fetched_at);
}
#[test]
fn only_bound_work_rating_url_is_accepted() {
    let f = Fixture::new();
    f.bind("897");
    let sources = f
        .backend
        .database()
        .unwrap()
        .list_external_sources(&f.game_id)
        .unwrap();
    let source = sources
        .iter()
        .find(|source| source.provider == "hikarinagi")
        .unwrap();
    assert!(allowed_rating_url(
        source,
        "https://www.hikarinagi.org/galgames/897/rates"
    ));
    for url in [
        "https://www.hikarinagi.org/galgames/898/rates",
        "https://evil.example/galgames/897/rates",
        "https://www.hikarinagi.org/galgames/897/rates?redirect=https://evil.example",
        "https://www.hikarinagi.org/galgames/897/rates/../../user",
    ] {
        assert!(!allowed_rating_url(source, url));
    }
}
#[test]
#[ignore = "explicit public-network smoke; no credentials or user database"]
fn live_public_statistics_are_fetched_and_cached() {
    let f = Fixture::new();
    f.bind("897");
    let response = get(&f.backend, f.request(true)).unwrap().wall.unwrap();
    assert_eq!(response.remote_id, "897");
    assert!(response.rated_count > 0);
    assert_eq!(
        response
            .distribution
            .iter()
            .map(|row| row.count)
            .sum::<u64>(),
        response.rated_count
    );
    assert!(!response.keywords.is_empty());
    assert!(
        get(&f.backend, f.request(false))
            .unwrap()
            .wall
            .unwrap()
            .cached
    );
}

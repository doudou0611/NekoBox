//! Request-scoped cancellation. No SQLite mutex is acquired by cancel.
use super::*;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Searches {
    active: HashMap<String, Arc<AtomicBool>>,
    cancelled: HashMap<String, Instant>,
}
impl Searches {
    fn prune(&mut self) {
        self.cancelled
            .retain(|_, at| at.elapsed() < Duration::from_secs(300));
    }
}
#[derive(serde::Deserialize)]
pub struct CancelRequest {
    pub search_request_id: String,
}
fn cancelled() -> ServiceError {
    ServiceError(ErrorCode::Cancelled, "资料搜索已取消。")
}
pub struct Scope {
    pub backend: Backend,
    id: String,
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Ok(mut searches) = self.backend.metadata_searches.lock() {
            searches.active.remove(&self.id);
        }
    }
}
pub fn begin(b: &Backend, request_id: &str) -> Result<Scope> {
    if !crate::domain::protocol::valid_request_id(request_id) {
        return Err(invalid("搜索请求标识无效。"));
    }
    let mut searches = b
        .metadata_searches
        .lock()
        .map_err(|_| invalid("搜索服务需要重启。"))?;
    searches.prune();
    if searches.cancelled.contains_key(request_id) {
        return Err(cancelled());
    }
    if searches.active.contains_key(request_id) {
        return Err(invalid("搜索请求标识不可重复。"));
    }
    let flag = Arc::new(AtomicBool::new(false));
    searches.active.insert(request_id.into(), flag.clone());
    let mut backend = b.clone();
    backend.metadata_search_cancelled = Some(flag);
    Ok(Scope {
        backend,
        id: request_id.into(),
    })
}
pub fn cancel(b: &Backend, q: CancelRequest) -> Result<bool> {
    if !crate::domain::protocol::valid_request_id(&q.search_request_id) {
        return Err(invalid("搜索请求标识无效。"));
    }
    let mut searches = b
        .metadata_searches
        .lock()
        .map_err(|_| invalid("搜索服务需要重启。"))?;
    searches.prune();
    // Reject excessive unknown cancellations rather than evicting a live tombstone.
    if searches.cancelled.len() >= 4096 && !searches.cancelled.contains_key(&q.search_request_id) {
        return Err(invalid("取消请求过多，请稍后重试。"));
    }
    if let Some(flag) = searches.active.get(&q.search_request_id) {
        flag.store(true, Ordering::Release);
    }
    searches
        .cancelled
        .insert(q.search_request_id, Instant::now());
    Ok(true)
}
impl Backend {
    pub(crate) fn ensure_search_active(&self) -> Result<()> {
        if self
            .metadata_search_cancelled
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire))
        {
            Err(cancelled())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (std::path::PathBuf, Backend) {
        let root = std::env::temp_dir().join(format!("gm-search-cancel-{}", id()));
        let b = Backend::open(root.clone()).unwrap();
        (root, b)
    }
    fn cancel_id(b: &Backend, id: &str) {
        cancel(
            b,
            CancelRequest {
                search_request_id: id.into(),
            },
        )
        .unwrap();
    }
    #[test]
    fn cancel_before_registration_is_remembered_and_invalid_ids_rejected() {
        let (root, b) = fixture();
        cancel_id(&b, "early");
        assert_eq!(begin(&b, "early").err().unwrap().0, ErrorCode::Cancelled);
        assert!(cancel(
            &b,
            CancelRequest {
                search_request_id: "../invalid".into()
            }
        )
        .is_err());
        assert!(begin(&b, "../invalid").is_err());
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancel_never_needs_database_and_scope_cleanup_is_isolated() {
        let (root, b) = fixture();
        let first = begin(&b, "first").unwrap();
        let second = begin(&b, "second").unwrap();
        assert!(begin(&b, "first").is_err());
        let db = b.database().unwrap();
        cancel_id(&b, "first");
        assert_eq!(
            first.backend.ensure_search_active().unwrap_err().0,
            ErrorCode::Cancelled
        );
        assert_eq!(
            first.backend.database().err().unwrap().0,
            ErrorCode::Cancelled
        );
        assert!(second.backend.ensure_search_active().is_ok());
        drop(db);
        drop(first);
        assert_eq!(b.metadata_searches.lock().unwrap().active.len(), 1);
        drop(second);
        assert!(b.metadata_searches.lock().unwrap().active.is_empty());
        assert!(b.database().unwrap().list_collections().is_ok());
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancelled_search_rejects_single_sources_and_shared_source_flow() {
        let (root, b) = fixture();
        for providers in [vec!["bangumi"], vec!["hikarinagi"], vec!["vndb"], vec![]] {
            let request_id = id();
            let scope = begin(&b, &request_id).unwrap();
            cancel_id(&b, &request_id);
            let query = crate::domain::requests::SearchMetadataRequest {
                manual: false,
                batch_id: None,
                query: "取消作品".into(),
                providers: providers.into_iter().map(str::to_owned).collect(),
                cache: true,
            };
            assert_eq!(
                metadata_sources::search(&scope.backend, &query)
                    .unwrap_err()
                    .0,
                ErrorCode::Cancelled
            );
        }
        assert!(b.metadata_searches.lock().unwrap().active.is_empty());
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
}

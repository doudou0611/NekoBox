//! Ordered, task-scoped source selection; no disabled provider is called implicitly.
use super::*;
use crate::domain::{
    models::{MatchResult, MetadataCandidate},
    requests::{ConfirmMetadataMatchRequest, SearchMetadataRequest},
};
use serde::{Deserialize, Serialize};
pub const SETTING: &str = "metadata.sources";
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub provider: String,
    pub enabled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub sources: Vec<Source>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            sources: ["hikarinagi", "bangumi", "vndb"]
                .into_iter()
                .map(|s| Source {
                    provider: s.into(),
                    enabled: s == "hikarinagi",
                })
                .collect(),
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        let mut names = self
            .sources
            .iter()
            .map(|s| s.provider.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        if names != ["bangumi", "hikarinagi", "vndb"] || !self.sources.iter().any(|s| s.enabled) {
            return Err(invalid("必须保留三个来源且至少启用一个来源。"));
        }
        Ok(())
    }
    pub fn enabled(&self) -> Vec<String> {
        self.sources
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.provider.clone())
            .collect()
    }
}
pub fn get(b: &Backend) -> Result<Config> {
    let config = if let Some(snapshot) = &b.metadata_snapshot {
        snapshot.clone()
    } else {
        b.database()?.setting(SETTING)?.unwrap_or_default()
    };
    config.validate()?;
    Ok(config)
}
pub fn save(b: &Backend, c: Config) -> Result<Config> {
    c.validate()?;
    b.database()?.put_setting(SETTING, &c)?;
    Ok(c)
}
fn search_one(b: &Backend, q: &SearchMetadataRequest) -> Result<Vec<MetadataCandidate>> {
    match q.providers[0].as_str() {
        "bangumi" => bangumi::search_metadata(b, q),
        "vndb" => vndb::search(b, q),
        "hikarinagi" => hikarinagi::search(b, q),
        _ => Err(invalid("未知资料来源。")),
    }
}
pub fn search(b: &Backend, q: &SearchMetadataRequest) -> Result<Vec<MetadataCandidate>> {
    b.ensure_search_active()?;
    let mut snapshot = b.clone();
    snapshot.metadata_snapshot = Some(get(b)?);
    if let Some(id) = &q.batch_id {
        snapshot.metadata_snapshot = Some(import_metadata::batch_config(b, id)?);
        snapshot.import_batch = Some(id.clone());
    }
    if q.manual {
        if q.providers.len() != 1
            || !["bangumi", "hikarinagi", "vndb"].contains(&q.providers[0].as_str())
        {
            return Err(invalid("手动搜索请选择一个资料源。"));
        }
        let mut config = get(&snapshot)?;
        for source in &mut config.sources {
            source.enabled = source.provider == q.providers[0];
        }
        snapshot.metadata_snapshot = Some(config);
    }
    let b = &snapshot;
    if q.providers.len() > 3
        || q.providers
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != q.providers.len()
    {
        return Err(invalid("最多选择三个不同的资料来源。"));
    }
    let enabled = get(b)?.enabled();
    if q.providers.iter().any(|p| !enabled.contains(p)) {
        return Err(invalid("该资料源已关闭，请在元数据设置中启用。"));
    }
    let providers = enabled
        .into_iter()
        .filter(|p| q.providers.is_empty() || q.providers.contains(p))
        .collect::<Vec<_>>();
    let mut result = vec![];
    let mut failure = None;
    for provider in providers {
        b.ensure_search_active()?;
        if let Some(id) = &b.import_batch {
            import_metadata::batch_config(b, id)?;
        }
        match search_one(
            b,
            &SearchMetadataRequest {
                manual: false,
                batch_id: None,
                query: q.query.clone(),
                providers: vec![provider],
                cache: q.cache,
            },
        ) {
            Ok(items) => result.extend(items),
            Err(e) => failure = Some(e),
        }
        b.ensure_search_active()?;
    }
    if result.is_empty() {
        if let Some(e) = failure {
            return Err(e);
        }
    }
    Ok(result)
}
fn exact(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
pub(crate) fn needs_cover(b: &Backend, game: &str, provider: &str) -> Result<bool> {
    if !b.metadata_supplement {
        return Ok(true);
    }
    let detail = b.database()?.get_game(game)?;
    let Some(path) = detail.summary.cover_url.as_deref() else {
        return Ok(true);
    };
    if import_metadata::validate_cover(b, path).is_err() {
        return Ok(true);
    }
    let order = get(b)?.enabled();
    let incoming = order.iter().position(|p| p == provider).unwrap_or(99);
    let owners = detail
        .metadata
        .iter()
        .filter(|f| {
            f.field == "cover_path"
                && serde_json::from_str::<String>(&f.value).ok().as_deref() == Some(path)
        })
        .collect::<Vec<_>>();
    if owners
        .iter()
        .any(|f| f.manually_edited || f.provider == "manual")
    {
        return Ok(false);
    }
    Ok(owners
        .iter()
        .filter_map(|f| order.iter().position(|p| p == &f.provider))
        .min()
        .is_some_and(|rank| incoming < rank))
}
fn apply(b: &Backend, q: &ConfirmMetadataMatchRequest) -> Result<MatchResult> {
    match q.provider.as_str() {
        "bangumi" => bangumi::confirm_metadata(b, q),
        "hikarinagi" => hikarinagi::confirm(b, q),
        "vndb" => vndb::confirm(b, q),
        _ => Err(invalid("未知资料来源。")),
    }
}
/// Explicit binding establishes identity. Other sources need one exact title/alias match.
pub fn confirm(b: &Backend, q: &ConfirmMetadataMatchRequest) -> Result<MatchResult> {
    let detail = b.database()?.get_game(&q.game_id)?;
    if q.provider == "hikarinagi"
        && (detail
            .metadata
            .iter()
            .any(|f| f.provider == "steam" && f.field == "title")
            || detail
                .summary
                .installations
                .iter()
                .any(|i| matches!(i.source, crate::domain::protocol::InstallSource::Steam)))
    {
        super::steam::bind_community(b, &q.game_id, &q.remote_id)?;
        return Ok(MatchResult {
            game_id: q.game_id.clone(),
            provider: q.provider.clone(),
            remote_id: q.remote_id.clone(),
            matched_at: now(),
            translation_message: None,
            supplementation_message: Some(
                "已关联 Hikarinagi 安利墙；Steam 主资料保持优先。".into(),
            ),
            cover_message: None,
        });
    }
    confirm_with(b, q, apply, search_one, !cfg!(test))
}
fn confirm_with(
    b: &Backend,
    q: &ConfirmMetadataMatchRequest,
    mut apply: impl FnMut(&Backend, &ConfirmMetadataMatchRequest) -> Result<MatchResult>,
    mut search_one: impl FnMut(&Backend, &SearchMetadataRequest) -> Result<Vec<MetadataCandidate>>,
    supplement: bool,
) -> Result<MatchResult> {
    confirm_erased(b, q, &mut apply, &mut search_one, supplement)
}
type ApplySource<'a> =
    dyn FnMut(&Backend, &ConfirmMetadataMatchRequest) -> Result<MatchResult> + 'a;
type SearchSource<'a> =
    dyn FnMut(&Backend, &SearchMetadataRequest) -> Result<Vec<MetadataCandidate>> + 'a;
fn confirm_erased(
    b: &Backend,
    q: &ConfirmMetadataMatchRequest,
    apply: &mut ApplySource<'_>,
    search_one: &mut SearchSource<'_>,
    supplement: bool,
) -> Result<MatchResult> {
    b.database()?.ensure_metadata_unlocked(&q.game_id)?;
    let mut config = get(b)?;
    if q.manual {
        if !["bangumi", "hikarinagi", "vndb"].contains(&q.provider.as_str()) {
            return Err(invalid("未知资料来源。"));
        }
        let index = config
            .sources
            .iter()
            .position(|s| s.provider == q.provider)
            .unwrap();
        let mut selected = config.sources.remove(index);
        selected.enabled = true;
        config.sources.insert(0, selected);
    }
    let enabled = config.enabled();
    if !enabled.contains(&q.provider) {
        return Err(invalid("该资料源已关闭。"));
    }
    let mut staged_db = Database::in_memory()?;
    let expected = {
        let db = b.database()?;
        let expected = serde_json::to_string(&db.get_game(&q.game_id)?.metadata)
            .map_err(|_| invalid("资料状态无效。"))?;
        staged_db.restore_verified_snapshot(&db)?;
        expected
    };
    let cache_before = staged_db.metadata_cache_snapshot()?;
    let mut stage = b.clone();
    stage.db = Arc::new(Mutex::new(staged_db));
    stage.metadata_snapshot = Some(config);
    stage.app_settings_snapshot = Some(app_settings::get(b)?);
    stage
        .database()?
        .set_metadata_priority(&q.game_id, &enabled)?;
    let previous_detail = stage.database()?.get_game(&q.game_id)?;
    let fallback_title = previous_detail.summary.title;
    let previous = previous_detail.metadata;
    let is_title = |field: &str| {
        matches!(
            field,
            "title" | "title_zh" | "title_en" | "title_ja" | "title_alt"
        )
    };
    let mut aliases = q
        .title_hint
        .iter()
        .filter(|s| !s.trim().is_empty() && s.chars().count() <= 200 && !s.contains('\\'))
        .cloned()
        .collect::<Vec<_>>();
    if aliases.is_empty() && !q.manual && fallback_title.chars().count() <= 200 {
        aliases.push(fallback_title);
    }
    let mut accepted = Vec::<String>::new();
    let mut result: Option<MatchResult> = None;
    let mut failure = None;
    let mut warnings = vec![];
    let providers = if supplement {
        enabled.clone()
    } else {
        vec![q.provider.clone()]
    };
    for provider in providers {
        if let Some(id) = &stage.import_batch {
            import_metadata::batch_config(&stage, id)?;
        }
        let selected = if provider == q.provider {
            Some(ConfirmMetadataMatchRequest {
                manual: false,
                title_hint: None,
                game_id: q.game_id.clone(),
                provider: provider.clone(),
                remote_id: q.remote_id.clone(),
            })
        } else {
            let mut selected = None;
            for alias in &aliases {
                match search_one(
                    &stage,
                    &SearchMetadataRequest {
                        manual: false,
                        batch_id: None,
                        query: alias.clone(),
                        providers: vec![provider.clone()],
                        cache: false,
                    },
                ) {
                    Ok(items) => {
                        let mut exact_matches = items
                            .into_iter()
                            .filter(|c| {
                                c.confidence >= 0.9
                                    && aliases.iter().any(|a| {
                                        exact(a) == exact(&c.title)
                                            || c.subtitle
                                                .as_ref()
                                                .is_some_and(|t| exact(a) == exact(t))
                                    })
                            })
                            .collect::<Vec<_>>();
                        exact_matches.sort_by(|a, b| a.remote_id.cmp(&b.remote_id));
                        exact_matches.dedup_by(|a, b| a.remote_id == b.remote_id);
                        if exact_matches.len() == 1 {
                            let candidate = exact_matches.remove(0);
                            selected = Some(ConfirmMetadataMatchRequest {
                                manual: false,
                                title_hint: None,
                                game_id: q.game_id.clone(),
                                provider: provider.clone(),
                                remote_id: candidate.remote_id,
                            });
                            break;
                        }
                    }
                    Err(e) => {
                        warnings.push(format!("{provider}：{}", e.1));
                        failure = Some(e);
                        break;
                    }
                }
            }
            selected
        };
        let Some(selected) = selected else {
            warnings.push(format!("{provider}：未找到唯一的同作品匹配，未补充资料。"));
            continue;
        };
        let before = stage.database()?.metadata_binding_revision(
            &q.game_id,
            &provider,
            &selected.remote_id,
        )?;
        let mut source_stage = stage.clone();
        // Covers obey the same source order and preserve manual fields as text.
        source_stage.metadata_supplement = true;
        let fetched = apply(&source_stage, &selected);
        let after = stage.database()?.metadata_binding_revision(
            &q.game_id,
            &provider,
            &selected.remote_id,
        )?;
        let fresh = !after.is_empty() && after != before;
        let fetched = match fetched {
            Ok(r) => r,
            Err(e) if fresh => {
                warnings.push(format!("{provider}：{}", e.1));
                MatchResult {
                    game_id: q.game_id.clone(),
                    provider: provider.clone(),
                    remote_id: selected.remote_id.clone(),
                    matched_at: now(),
                    translation_message: None,
                    supplementation_message: None,
                    cover_message: None,
                }
            }
            Err(e) => {
                warnings.push(format!("{provider}：{}", e.1));
                failure = Some(e);
                continue;
            }
        };
        let detail = stage.database()?.get_game(&q.game_id)?;
        let titles = detail
            .metadata
            .iter()
            .filter(|f| f.provider == provider && !f.manually_edited && is_title(&f.field))
            .filter_map(|f| serde_json::from_str::<String>(&f.value).ok())
            .collect::<Vec<_>>();
        if accepted.is_empty() {
            let same_identity = previous
                .iter()
                .filter(|f| !f.manually_edited && is_title(&f.field))
                .filter_map(|f| serde_json::from_str::<String>(&f.value).ok())
                .any(|old| titles.iter().any(|new| exact(&old) == exact(new)));
            if !same_identity {
                let mut old_providers = previous
                    .iter()
                    .filter(|f| {
                        !f.manually_edited && f.provider != provider && f.provider != "hikarifield"
                    })
                    .map(|f| f.provider.clone())
                    .collect::<Vec<_>>();
                old_providers.sort();
                old_providers.dedup();
                for old in old_providers {
                    stage.database()?.unbind_metadata(&q.game_id, &old)?;
                }
            }
        }
        aliases.extend(titles.into_iter().filter(|s| !s.trim().is_empty()));
        aliases.sort();
        aliases.dedup();
        accepted.push(provider);
        if result.is_none() {
            result = Some(fetched);
        }
    }
    let mut result = result.ok_or_else(|| failure.unwrap_or_else(missing))?;
    let detail = stage.database()?.get_game(&q.game_id)?;
    result.cover_message = Some(
        if detail
            .summary
            .cover_url
            .as_deref()
            .is_some_and(|p| import_metadata::validate_cover(&stage, p).is_ok())
        {
            "封面已缓存。".into()
        } else {
            "资料已绑定；封面未获取，请重新匹配重试。".into()
        },
    );
    result.supplementation_message = (!warnings.is_empty()).then(|| warnings.join("；"));
    let tag_limit = app_settings::get(&stage)?.tag_limit;
    stage.database()?.limit_source_tags(&q.game_id, tag_limit)?;
    if !cfg!(test) {
        super::translation::complete(&stage, q, &mut result);
    }

    if let Some(id) = &stage.import_batch {
        import_metadata::batch_config(&stage, id)?;
    }
    b.database()?.commit_metadata_refresh_with_cache(
        &q.game_id,
        &expected,
        &*stage.database()?,
        &enabled,
        Some(&cache_before),
    )?;
    Ok(result)
}

#[cfg(test)]
mod tests;

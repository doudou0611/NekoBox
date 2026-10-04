use super::Database;
use crate::{
    backend::{self, types::BindSourceRequest, Result},
    domain::models::ExternalSource,
};
use rusqlite::params;

fn push_unique(items: &mut Vec<ExternalSource>, source: ExternalSource) {
    if !items.iter().any(|item| item.url == source.url) {
        items.push(source);
    }
}

impl Database {
    pub fn bind_external_source(
        &mut self,
        request: &BindSourceRequest,
    ) -> Result<Vec<ExternalSource>> {
        self.get_game(&request.game_id)?;
        self.ensure_metadata_unlocked(&request.game_id)?;
        match request.provider.as_str() {
            "steam" => {
                let id = request
                    .install_id
                    .as_ref()
                    .ok_or_else(|| backend::invalid("请选择要绑定的安装版本。"))?;
                let install = self.installation(id)?;
                if install.game_id != request.game_id {
                    return Err(backend::invalid("安装版本不属于该作品。"));
                }
                let value = request.value.trim();
                let app_id = if value.is_empty() {
                    None
                } else {
                    Some(
                        value
                            .parse::<i64>()
                            .map_err(|_| backend::invalid("Steam AppID 必须是正整数。"))?,
                    )
                };
                if app_id.is_some_and(|id| id <= 0) {
                    return Err(backend::invalid("Steam AppID 必须是正整数。"));
                }
                self.connection.execute(
                    "UPDATE game_installations SET steam_app_id=?1,updated_at=?2 WHERE id=?3",
                    params![app_id, backend::now(), id],
                )?;
            }
            "dlsite" => {
                let value = request.value.trim();
                if value.is_empty() {
                    self.connection.execute("DELETE FROM metadata_records WHERE game_id=? AND provider='dlsite' AND field_name='official_product'", [&request.game_id])?;
                } else {
                    let remote_id = dlsite_product_id(value)
                        .ok_or_else(|| backend::invalid("请粘贴 DLsite 官方商品 HTTPS 地址。"))?;
                    self.connection.execute("INSERT INTO metadata_records(id,game_id,provider,remote_id,field_name,value_json,source_url,is_user_edited) VALUES(?1,?2,'dlsite',?3,'official_product',?4,?5,1) ON CONFLICT(game_id,provider,field_name) DO UPDATE SET remote_id=excluded.remote_id,value_json=excluded.value_json,source_url=excluded.source_url,is_user_edited=1",params![backend::id(),request.game_id,remote_id,serde_json::to_string(value).map_err(|_| backend::invalid("链接无效。"))?,value])?;
                }
            }
            _ => return Err(backend::invalid("仅支持 Steam 和 DLsite 人工来源绑定。")),
        }
        self.list_external_sources(&request.game_id)
    }

    pub fn list_external_sources(&self, game_id: &str) -> Result<Vec<ExternalSource>> {
        self.get_game(game_id)?;
        let mut items = Vec::new();
        let mut installations = self.connection.prepare(
            "SELECT steam_app_id FROM game_installations WHERE game_id=? AND steam_app_id IS NOT NULL ORDER BY created_at,id",
        )?;
        for app_id in installations.query_map([game_id], |row| row.get::<_, Option<i64>>(0))? {
            if let Some(app_id) = app_id?.filter(|id| *id > 0) {
                push_unique(
                    &mut items,
                    ExternalSource {
                        kind: "steam".into(),
                        label: "Steam 官方商店".into(),
                        provider: "steam".into(),
                        url: format!("https://store.steampowered.com/app/{app_id}/"),
                        remote_id: Some(app_id.to_string()),
                        official: true,
                    },
                );
            }
        }
        let mut metadata = self.connection.prepare(
            "SELECT provider,remote_id,source_url FROM metadata_records WHERE game_id=? AND remote_id IS NOT NULL ORDER BY provider,field_name",
        )?;
        for row in metadata.query_map([game_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })? {
            let (provider, remote_id, source_url) = row?;
            let Some(remote_id) = remote_id.filter(|id| !id.trim().is_empty()) else {
                continue;
            };
            match provider.as_str() {
                "vndb" => {
                    let id = remote_id.trim();
                    if id
                        .strip_prefix('v')
                        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                    {
                        push_unique(
                            &mut items,
                            ExternalSource {
                                kind: "vndb".into(),
                                label: "VNDB 作品页".into(),
                                provider,
                                url: format!("https://vndb.org/{id}"),
                                remote_id: Some(id.into()),
                                official: true,
                            },
                        );
                    }
                }
                "bangumi" => {
                    let id = remote_id.trim();
                    if id.chars().all(|c| c.is_ascii_digit()) {
                        push_unique(
                            &mut items,
                            ExternalSource {
                                kind: "bangumi".into(),
                                label: "Bangumi 作品页".into(),
                                provider,
                                url: format!("https://bgm.tv/subject/{id}"),
                                remote_id: Some(id.into()),
                                official: true,
                            },
                        );
                    }
                }
                "hikarinagi" => {
                    let id = remote_id.trim();
                    if !id.is_empty()
                        && id.len() <= 20
                        && id.bytes().all(|byte| byte.is_ascii_digit())
                        && id.bytes().any(|byte| byte != b'0')
                    {
                        push_unique(
                            &mut items,
                            ExternalSource {
                                kind: "hikarinagi".into(),
                                label: "Hikarinagi 作品页".into(),
                                provider,
                                url: format!("https://www.hikarinagi.org/galgames/{id}"),
                                remote_id: Some(id.into()),
                                official: true,
                            },
                        );
                    }
                }
                "dlsite" => {
                    if let Some(url) = source_url.filter(|url| is_dlsite_url(url)) {
                        push_unique(
                            &mut items,
                            ExternalSource {
                                kind: "dlsite".into(),
                                label: "DLsite 官方商品".into(),
                                provider,
                                url,
                                remote_id: Some(remote_id),
                                official: true,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(items)
    }
}

fn dlsite_product_id(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value).ok()?;
    let host = url.host_str()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || !(host == "dlsite.com" || host.ends_with(".dlsite.com"))
    {
        return None;
    }
    let id = url
        .path()
        .split("/product_id/")
        .nth(1)?
        .strip_suffix(".html")?;
    let digits = id
        .strip_prefix("RJ")
        .or_else(|| id.strip_prefix("VJ"))
        .or_else(|| id.strip_prefix("BJ"))?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(id.to_owned())
}
fn is_dlsite_url(value: &str) -> bool {
    dlsite_product_id(value).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend, domain::protocol::InstallSource};

    #[test]
    fn dlsite_urls_reject_credentials_redirect_hosts_and_non_products() {
        for invalid in [
            "http://www.dlsite.com/maniax/work/=/product_id/RJ123.html",
            "https://user@www.dlsite.com/maniax/work/=/product_id/RJ123.html",
            "https://dlsite.com.evil.test/maniax/work/=/product_id/RJ123.html",
            "https://www.dlsite.com/",
            "https://www.dlsite.com/maniax/work/=/product_id/RJ.html",
        ] {
            assert!(!is_dlsite_url(invalid));
        }
        assert!(is_dlsite_url(
            "https://www.dlsite.com/maniax/work/=/product_id/RJ123.html"
        ));
    }
    #[test]
    fn binding_validates_ownership_and_preserves_launch_configuration() {
        let mut db = Database::in_memory().unwrap();
        let game = db
            .import_installation(
                "/fixture/source",
                "作品",
                None,
                InstallSource::Manual,
                &[],
                "sources",
            )
            .unwrap();
        let install = db.get_game(&game).unwrap().summary.installations[0]
            .id
            .clone();
        let mut request = BindSourceRequest {
            game_id: game.clone(),
            install_id: Some(install.clone()),
            provider: "steam".into(),
            value: "123".into(),
        };
        assert_eq!(
            db.bind_external_source(&request).unwrap()[0]
                .remote_id
                .as_deref(),
            Some("123")
        );
        assert!(db.installation(&install).unwrap().executable_path.is_none());
        request.value = "-1".into();
        assert!(db.bind_external_source(&request).is_err());
        assert_eq!(
            db.installation(&install).unwrap().steam_app_id.as_deref(),
            Some("123")
        );
        request.value.clear();
        assert!(db.bind_external_source(&request).unwrap().is_empty());
        request.provider = "dlsite".into();
        request.value = "https://www.dlsite.com/maniax/work/=/product_id/RJ123.html".into();
        assert_eq!(db.bind_external_source(&request).unwrap()[0].kind, "dlsite");
        request.value.clear();
        assert!(db.bind_external_source(&request).unwrap().is_empty());
    }
    #[test]
    fn derives_only_official_sources() {
        let mut db = Database::in_memory().unwrap();
        let root = std::env::temp_dir().join(format!("gm-sources-{}", backend::id()));
        std::fs::create_dir_all(&root).unwrap();
        let game = db
            .import_installation(
                root.to_str().unwrap(),
                "作品",
                None,
                InstallSource::Steam,
                &[],
                "sources",
            )
            .unwrap();
        let install = db.get_game(&game).unwrap().summary.installations[0]
            .id
            .clone();
        db.connection
            .execute(
                "UPDATE game_installations SET steam_app_id=12345 WHERE id=?",
                [&install],
            )
            .unwrap();
        db.connection.execute("INSERT INTO metadata_records(id,game_id,provider,remote_id,field_name,value_json,source_url) VALUES('v',?,'vndb','v123','title','\"作品\"','https://vndb.org/v123')", [&game]).unwrap();
        db.connection.execute("INSERT INTO metadata_records(id,game_id,provider,remote_id,field_name,value_json,source_url) VALUES('b',?,'bangumi','123','cover_url','\"封面\"','https://bgm.tv/subject/123')", [&game]).unwrap();
        db.connection.execute("INSERT INTO metadata_records(id,game_id,provider,remote_id,field_name,value_json,source_url) VALUES('d',?,'dlsite','RJ0001','title','\"商品\"','https://example.invalid/RJ0001')", [&game]).unwrap();
        let items = db.list_external_sources(&game).unwrap();
        assert_eq!(items.len(), 3);
        assert!(items.iter().any(|item| item.kind == "steam"));
        std::fs::remove_dir_all(root).unwrap();
    }
}

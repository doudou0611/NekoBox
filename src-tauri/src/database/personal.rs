use super::Database;
use crate::{
    backend,
    backend::{
        types::{NoteRequest, ReplaceGameTagsRequest, UpdateScreenshotRequest},
        Result,
    },
    domain::models::{Note, Screenshot, Tag},
};
use rusqlite::{params, OptionalExtension};

impl Database {
    pub(crate) fn screenshot_has_other_references(&self, path: &str, id: &str) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM screenshots WHERE absolute_path=?1 AND id!=?2)",
            params![path, id],
            |r| r.get(0),
        )?)
    }

    pub(crate) fn delete_screenshot_record(&self, id: &str, game: &str) -> Result<()> {
        if self.connection.execute(
            "DELETE FROM screenshots WHERE id=?1 AND game_id=?2",
            params![id, game],
        )? != 1
        {
            return Err(backend::missing());
        }
        Ok(())
    }

    pub fn list_screenshots(
        &self,
        game_id: &str,
        include_spoilers: bool,
    ) -> Result<Vec<Screenshot>> {
        self.get_game(game_id)?;
        let mut sql = String::from("SELECT id,game_id,install_id,absolute_path,title,is_spoiler,captured_at,created_at,thumbnail_path FROM screenshots WHERE game_id=?");
        if !include_spoilers {
            sql.push_str(" AND is_spoiler=0");
        }
        sql.push_str(" ORDER BY coalesce(captured_at,created_at) DESC,id DESC");
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map([game_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get(2)?,
                r.get::<_, String>(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })?;
        rows.map(|row| {
            let (id, game_id, install_id, path, title, spoiler, captured_at, created_at,thumbnail) = row?;
            Ok(Screenshot {
                id,
                game_id,
                install_id,
                image_url: path.clone(),
                thumbnail_url: thumbnail.unwrap_or(path),
                title,
                is_spoiler: spoiler,
                captured_at,
                created_at,
            })
        })
        .collect::<Result<Vec<_>>>()
    }
    pub fn screenshot_path(&self, id: &str) -> Result<Option<(String, String)>> {
        Ok(self
            .connection
            .query_row(
                "SELECT absolute_path,game_id FROM screenshots WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }
    pub fn upsert_screenshot(
        &self,
        game_id: &str,
        install_id: &str,
        path: &str,
        captured_at: Option<&str>,
    ) -> Result<()> {
        self.connection.execute("INSERT INTO screenshots(id,game_id,install_id,absolute_path,captured_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(game_id,absolute_path) DO UPDATE SET install_id=excluded.install_id,captured_at=coalesce(excluded.captured_at,screenshots.captured_at)", params![backend::id(),game_id,install_id,path,captured_at])?;
        Ok(())
    }
    pub fn set_screenshot_thumbnail(
        &self,
        game_id: &str,
        path: &str,
        thumbnail: &str,
    ) -> Result<()> {
        self.connection.execute(
            "UPDATE screenshots SET thumbnail_path=?1 WHERE game_id=?2 AND absolute_path=?3",
            params![thumbnail, game_id, path],
        )?;
        Ok(())
    }
    pub fn screenshot_thumbnail(&self, id: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                "SELECT thumbnail_path FROM screenshots WHERE id=?",
                [id],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }
    pub fn update_screenshot(&self, r: &UpdateScreenshotRequest) -> Result<Screenshot> {
        if r.title.chars().count() > 120 || r.title.contains('\0') {
            return Err(backend::invalid("截图标题最多 120 个字符。"));
        }
        let captured = r
            .captured_at
            .as_ref()
            .map(|value| {
                chrono::DateTime::parse_from_rfc3339(value)
                    .map(|date| {
                        date.with_timezone(&chrono::Utc)
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                    })
                    .map_err(|_| backend::invalid("截图拍摄时间格式无效。"))
            })
            .transpose()?;
        if self.connection.execute("UPDATE screenshots SET title=?1,captured_at=?2,is_spoiler=?3 WHERE id=?4 AND game_id=?5",params![r.title.trim(),captured,r.is_spoiler,r.screenshot_id,r.game_id])? == 0 {
            return Err(backend::missing());
        }
        self.list_screenshots(&r.game_id, true)?
            .into_iter()
            .find(|item| item.id == r.screenshot_id)
            .ok_or_else(backend::missing)
    }
    pub fn list_notes(&self, game_id: &str) -> Result<Vec<Note>> {
        self.get_game(game_id)?;
        let mut statement = self.connection.prepare("SELECT id,game_id,title,content_markdown,is_spoiler,created_at,updated_at FROM notes WHERE game_id=? ORDER BY updated_at DESC,id DESC")?;
        let rows = statement.query_map([game_id], |r| {
            Ok(Note {
                id: r.get(0)?,
                game_id: r.get(1)?,
                title: r.get(2)?,
                content_markdown: r.get(3)?,
                is_spoiler: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
    pub fn save_note(&self, request: &NoteRequest) -> Result<Note> {
        self.get_game(&request.game_id)?;
        if request.title.chars().count() > 120 || request.content_markdown.chars().count() > 200_000
        {
            return Err(backend::invalid("笔记标题或内容超过限制。"));
        }
        let id = request.id.clone().unwrap_or_else(backend::id);
        if self.connection.execute("INSERT INTO notes(id,game_id,title,content_markdown,is_spoiler) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET title=excluded.title,content_markdown=excluded.content_markdown,is_spoiler=excluded.is_spoiler,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE notes.game_id=excluded.game_id", params![id,request.game_id,request.title.trim(),request.content_markdown,request.is_spoiler])? == 0 {return Err(backend::missing());}
        self.connection.query_row("SELECT id,game_id,title,content_markdown,is_spoiler,created_at,updated_at FROM notes WHERE id=?", [&id], |r| Ok(Note { id:r.get(0)?,game_id:r.get(1)?,title:r.get(2)?,content_markdown:r.get(3)?,is_spoiler:r.get(4)?,created_at:r.get(5)?,updated_at:r.get(6)? })).map_err(Into::into)
    }
    pub fn delete_note(&self, id: &str) -> Result<bool> {
        Ok(self
            .connection
            .execute("DELETE FROM notes WHERE id=?", [id])?
            != 0)
    }
    pub fn replace_game_tags(&mut self, request: &ReplaceGameTagsRequest) -> Result<Vec<Tag>> {
        self.get_game(&request.game_id)?;
        if request.tag_names.len() > 50 {
            return Err(backend::invalid("标签数量不能超过 50 个。"));
        }
        let mut names = Vec::new();
        for raw in &request.tag_names {
            let name = raw.trim();
            if name.is_empty() || name.chars().count() > 40 {
                return Err(backend::invalid("标签名称需要 1～40 个字符。"));
            }
            if !names
                .iter()
                .any(|value: &String| value.eq_ignore_ascii_case(name))
            {
                names.push(name.to_owned());
            }
        }
        let tx = self.connection.transaction()?;
        tx.execute("DELETE FROM game_tags WHERE game_id=?", [&request.game_id])?;
        for name in &names {
            tx.execute(
                "INSERT INTO tags(id,name) VALUES(?1,?2) ON CONFLICT(name) DO NOTHING",
                params![backend::id(), name],
            )?;
            let tag_id: String = tx.query_row(
                "SELECT id FROM tags WHERE name=? COLLATE NOCASE",
                [name],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO game_tags(id,game_id,tag_id) VALUES(?1,?2,?3)",
                params![backend::id(), request.game_id, tag_id],
            )?;
        }
        tx.commit()?;
        let mut statement = self.connection.prepare("SELECT t.id,t.name,t.color FROM tags t JOIN game_tags gt ON gt.tag_id=t.id WHERE gt.game_id=? ORDER BY t.name")?;
        let rows = statement.query_map([&request.game_id], |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend, domain::protocol::InstallSource};
    use std::fs;

    #[test]
    fn notes_tags_rating_and_screenshot_records_are_scoped() {
        let mut db = Database::in_memory().unwrap();
        let root = std::env::temp_dir().join(format!("gm-personal-{}", backend::id()));
        fs::create_dir_all(root.join("game")).unwrap();
        let game = db
            .import_installation(
                root.join("game").to_str().unwrap(),
                "作品",
                None,
                InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        let note = db
            .save_note(&NoteRequest {
                id: None,
                game_id: game.clone(),
                title: "路线".into(),
                content_markdown: "# 记录".into(),
                is_spoiler: true,
            })
            .unwrap();
        assert_eq!(db.list_notes(&game).unwrap()[0].id, note.id);
        fs::create_dir_all(root.join("other")).unwrap();
        let other = db
            .import_installation(
                root.join("other").to_str().unwrap(),
                "其他作品",
                None,
                InstallSource::Manual,
                &[],
                "fixture",
            )
            .unwrap();
        assert!(db
            .save_note(&NoteRequest {
                id: Some(note.id.clone()),
                game_id: other,
                title: "越界".into(),
                content_markdown: "不应保存".into(),
                is_spoiler: false
            })
            .is_err());
        assert_eq!(db.list_notes(&game).unwrap()[0].content_markdown, "# 记录");
        let tags = db
            .replace_game_tags(&ReplaceGameTagsRequest {
                game_id: game.clone(),
                tag_names: vec!["治愈".into(), "治愈".into(), "音乐".into()],
            })
            .unwrap();
        assert_eq!(tags.len(), 2);
        db.upsert_screenshot(
            &game,
            &db.get_game(&game).unwrap().summary.installations[0].id,
            root.join("game/shot.png").to_str().unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(db.list_screenshots(&game, false).unwrap().len(), 1);
        db.delete_note(&note.id).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

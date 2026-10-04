-- Only called by Rust rollback_empty after EVERY domain table has been checked empty.
-- Production rollback is backup + restore, not automatic destructive SQL.
DROP TRIGGER collections_kind_guard;
DROP TRIGGER collection_members_normal_update;
DROP TRIGGER collection_members_normal_insert;
DROP TABLE settings;
DROP TABLE recommendation_preferences;
DROP TABLE collection_members;
DROP TABLE collections;
DROP TABLE smart_filters;
DROP TABLE notes;
DROP TABLE game_tags;
DROP TABLE tags;
DROP TABLE screenshots;
DROP TABLE save_snapshots;
DROP TABLE save_profiles;
DROP TABLE play_sessions;
DROP TABLE aliases;
DROP TABLE metadata_cache;
DROP TABLE metadata_records;
DROP TABLE game_installations;
DROP TABLE games;

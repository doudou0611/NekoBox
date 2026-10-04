-- AI-02 schema v1. All files here are immutable after a release.
-- UTC timestamps use ISO-8601 TEXT ending in Z; all durations are integer seconds.
CREATE TABLE games (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    title_zh TEXT,
    title_ja TEXT,
    title_en TEXT,
    description TEXT,
    cover_path TEXT,
    developer TEXT,
    release_date TEXT,
    status TEXT NOT NULL DEFAULT 'not_started' CHECK (status IN ('not_started', 'playing', 'paused', 'completed', 'dropped', 'pending_confirmation')),
    is_favorite INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),
    is_hidden INTEGER NOT NULL DEFAULT 0 CHECK (is_hidden IN (0, 1)),
    user_rating REAL CHECK (user_rating BETWEEN 0 AND 10),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at GLOB '????-??-??T??:??:??*Z' AND julianday(updated_at) IS NOT NULL)
) STRICT;

CREATE TABLE game_installations (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    absolute_path TEXT NOT NULL UNIQUE CHECK (length(trim(absolute_path)) > 0),
    folder_name TEXT NOT NULL,
    executable_path TEXT,
    launch_arguments_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(launch_arguments_json) AND json_type(launch_arguments_json) = 'array'),
    working_directory TEXT,
    environment_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(environment_json) AND json_type(environment_json) = 'object'),
    run_as_admin INTEGER NOT NULL DEFAULT 0 CHECK (run_as_admin IN (0, 1)),
    track_after_launcher_exit INTEGER NOT NULL DEFAULT 1 CHECK (track_after_launcher_exit IN (0, 1)),
    main_process_name TEXT,
    source TEXT NOT NULL DEFAULT 'local' CHECK (source IN ('local', 'steam', 'manual', 'unknown')),
    steam_app_id INTEGER CHECK (steam_app_id > 0),
    product_name TEXT,
    company_name TEXT,
    fingerprint TEXT,
    scan_status TEXT NOT NULL DEFAULT 'pending' CHECK (scan_status IN ('pending', 'scanned', 'error', 'ignored')),
    scanned_at TEXT CHECK (scanned_at IS NULL OR (scanned_at GLOB '????-??-??T??:??:??*Z' AND julianday(scanned_at) IS NOT NULL)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at GLOB '????-??-??T??:??:??*Z' AND julianday(updated_at) IS NOT NULL),
    UNIQUE (id, game_id)
) STRICT;

-- Field-level provenance: one record per provider and field, not a second game entity.
CREATE TABLE metadata_records (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (length(trim(provider)) > 0),
    remote_id TEXT,
    field_name TEXT NOT NULL CHECK (length(trim(field_name)) > 0),
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    source_url TEXT,
    is_user_edited INTEGER NOT NULL DEFAULT 0 CHECK (is_user_edited IN (0, 1)),
    fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (fetched_at GLOB '????-??-??T??:??:??*Z' AND julianday(fetched_at) IS NOT NULL),
    UNIQUE (game_id, provider, field_name)
) STRICT;

CREATE TABLE metadata_cache (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    provider TEXT NOT NULL CHECK (length(trim(provider)) > 0),
    query TEXT NOT NULL,
    response_json TEXT CHECK (response_json IS NULL OR json_valid(response_json)),
    status TEXT NOT NULL CHECK (status IN ('pending', 'success', 'error')),
    error_message TEXT,
    fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (fetched_at GLOB '????-??-??T??:??:??*Z' AND julianday(fetched_at) IS NOT NULL),
    expires_at TEXT CHECK (expires_at IS NULL OR (expires_at GLOB '????-??-??T??:??:??*Z' AND julianday(expires_at) IS NOT NULL)),
    UNIQUE (provider, query),
    CHECK (status <> 'success' OR response_json IS NOT NULL),
    CHECK (expires_at IS NULL OR julianday(expires_at) >= julianday(fetched_at))
) STRICT;

CREATE TABLE aliases (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    alias TEXT NOT NULL CHECK (length(trim(alias)) > 0),
    language TEXT NOT NULL DEFAULT 'und',
    provider TEXT NOT NULL DEFAULT 'manual',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    UNIQUE (game_id, alias, language)
) STRICT;

CREATE TABLE play_sessions (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    install_id TEXT NOT NULL,
    started_at TEXT NOT NULL CHECK (started_at GLOB '????-??-??T??:??:??*Z' AND julianday(started_at) IS NOT NULL),
    ended_at TEXT CHECK (ended_at IS NULL OR (ended_at GLOB '????-??-??T??:??:??*Z' AND julianday(ended_at) IS NOT NULL)),
    duration_seconds INTEGER NOT NULL DEFAULT 0 CHECK (duration_seconds >= 0),
    process_name TEXT,
    end_reason TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    CHECK (ended_at IS NULL OR julianday(ended_at) >= julianday(started_at)),
    FOREIGN KEY (install_id, game_id) REFERENCES game_installations(id, game_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE save_profiles (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    install_id TEXT NOT NULL,
    source_path TEXT NOT NULL CHECK (length(trim(source_path)) > 0),
    target_path TEXT NOT NULL CHECK (length(trim(target_path)) > 0),
    include_patterns_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(include_patterns_json) AND json_type(include_patterns_json) = 'array'),
    exclude_patterns_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(exclude_patterns_json) AND json_type(exclude_patterns_json) = 'array'),
    backup_before_launch INTEGER NOT NULL DEFAULT 0 CHECK (backup_before_launch IN (0, 1)),
    backup_after_exit INTEGER NOT NULL DEFAULT 0 CHECK (backup_after_exit IN (0, 1)),
    retention_count INTEGER NOT NULL DEFAULT 10 CHECK (retention_count > 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at GLOB '????-??-??T??:??:??*Z' AND julianday(updated_at) IS NOT NULL),
    UNIQUE (install_id, source_path),
    FOREIGN KEY (install_id, game_id) REFERENCES game_installations(id, game_id) ON DELETE CASCADE
) STRICT;

-- The profile is the single source of ownership; no redundant game/install ID here.
CREATE TABLE save_snapshots (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    profile_id TEXT NOT NULL REFERENCES save_profiles(id) ON DELETE CASCADE,
    archive_path TEXT NOT NULL UNIQUE CHECK (length(trim(archive_path)) > 0),
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    file_count INTEGER NOT NULL DEFAULT 0 CHECK (file_count >= 0),
    label TEXT,
    note TEXT,
    creation_reason TEXT NOT NULL DEFAULT 'manual',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL)
) STRICT;

CREATE TABLE screenshots (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    install_id TEXT,
    absolute_path TEXT NOT NULL CHECK (length(trim(absolute_path)) > 0),
    thumbnail_path TEXT,
    title TEXT,
    is_spoiler INTEGER NOT NULL DEFAULT 0 CHECK (is_spoiler IN (0, 1)),
    captured_at TEXT CHECK (captured_at IS NULL OR (captured_at GLOB '????-??-??T??:??:??*Z' AND julianday(captured_at) IS NOT NULL)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    UNIQUE (game_id, absolute_path),
    FOREIGN KEY (install_id, game_id) REFERENCES game_installations(id, game_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE tags (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    name TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK (length(trim(name)) > 0),
    color TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL)
) STRICT;

CREATE TABLE game_tags (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    UNIQUE (game_id, tag_id)
) STRICT;

CREATE TABLE notes (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT '',
    content_markdown TEXT NOT NULL DEFAULT '',
    is_spoiler INTEGER NOT NULL DEFAULT 1 CHECK (is_spoiler IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at GLOB '????-??-??T??:??:??*Z' AND julianday(updated_at) IS NOT NULL)
) STRICT;

CREATE TABLE smart_filters (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    query_json TEXT NOT NULL CHECK (json_valid(query_json) AND json_type(query_json) = 'object'),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at GLOB '????-??-??T??:??:??*Z' AND julianday(updated_at) IS NOT NULL)
) STRICT;

CREATE TABLE collections (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    kind TEXT NOT NULL CHECK (kind IN ('normal', 'smart')),
    smart_filter_id TEXT REFERENCES smart_filters(id) ON DELETE RESTRICT,
    icon TEXT,
    color TEXT,
    cover_path TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0 CHECK (sort_order >= 0),
    is_hidden INTEGER NOT NULL DEFAULT 0 CHECK (is_hidden IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at GLOB '????-??-??T??:??:??*Z' AND julianday(updated_at) IS NOT NULL),
    CHECK ((kind = 'normal' AND smart_filter_id IS NULL) OR (kind = 'smart' AND smart_filter_id IS NOT NULL))
) STRICT;

CREATE TABLE collection_members (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0 CHECK (position >= 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    UNIQUE (collection_id, game_id)
) STRICT;

CREATE TRIGGER collection_members_normal_insert
BEFORE INSERT ON collection_members
WHEN COALESCE((SELECT kind FROM collections WHERE id = NEW.collection_id), '') <> 'normal'
BEGIN
    SELECT RAISE(ABORT, 'collection_members require a normal collection');
END;

CREATE TRIGGER collection_members_normal_update
BEFORE UPDATE OF collection_id ON collection_members
WHEN COALESCE((SELECT kind FROM collections WHERE id = NEW.collection_id), '') <> 'normal'
BEGIN
    SELECT RAISE(ABORT, 'collection_members require a normal collection');
END;

CREATE TRIGGER collections_kind_guard
BEFORE UPDATE OF kind ON collections
WHEN NEW.kind = 'smart' AND EXISTS (SELECT 1 FROM collection_members WHERE collection_id = OLD.id)
BEGIN
    SELECT RAISE(ABORT, 'remove normal collection members before changing kind');
END;

CREATE TABLE recommendation_preferences (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    preference TEXT NOT NULL CHECK (preference IN ('not_interested', 'snoozed')),
    expires_at TEXT CHECK (expires_at IS NULL OR (expires_at GLOB '????-??-??T??:??:??*Z' AND julianday(expires_at) IS NOT NULL)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (created_at GLOB '????-??-??T??:??:??*Z' AND julianday(created_at) IS NOT NULL),
    UNIQUE (game_id, preference),
    CHECK ((preference = 'not_interested' AND expires_at IS NULL) OR (preference = 'snoozed' AND expires_at IS NOT NULL))
) STRICT;

CREATE TABLE settings (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    key TEXT NOT NULL UNIQUE CHECK (length(trim(key)) > 0),
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) CHECK (updated_at GLOB '????-??-??T??:??:??*Z' AND julianday(updated_at) IS NOT NULL)
) STRICT;

CREATE INDEX games_status_idx ON games(status, is_hidden);
CREATE INDEX games_favorite_idx ON games(is_favorite, is_hidden);
CREATE INDEX games_created_at_idx ON games(created_at);
CREATE INDEX game_installations_game_idx ON game_installations(game_id);
CREATE INDEX game_installations_source_idx ON game_installations(source);
CREATE INDEX metadata_records_game_idx ON metadata_records(game_id);
CREATE INDEX metadata_cache_expiry_idx ON metadata_cache(expires_at);
CREATE INDEX aliases_lookup_idx ON aliases(alias COLLATE NOCASE);
CREATE INDEX play_sessions_game_started_idx ON play_sessions(game_id, started_at);
CREATE INDEX play_sessions_install_game_idx ON play_sessions(install_id, game_id);
CREATE INDEX play_sessions_open_idx ON play_sessions(started_at) WHERE ended_at IS NULL;
CREATE INDEX save_profiles_game_idx ON save_profiles(game_id);
CREATE INDEX save_snapshots_profile_created_idx ON save_snapshots(profile_id, created_at);
CREATE INDEX screenshots_game_idx ON screenshots(game_id, captured_at);
CREATE INDEX screenshots_install_game_idx ON screenshots(install_id, game_id);
CREATE INDEX game_tags_tag_idx ON game_tags(tag_id);
CREATE INDEX notes_game_idx ON notes(game_id, updated_at);
CREATE INDEX collections_filter_idx ON collections(smart_filter_id);
CREATE INDEX collections_sort_idx ON collections(is_hidden, sort_order);
CREATE INDEX collection_members_game_idx ON collection_members(game_id);
CREATE INDEX collection_members_position_idx ON collection_members(collection_id, position);
CREATE INDEX recommendation_preferences_expiry_idx ON recommendation_preferences(expires_at);

-- Migration 0002 — domínio musical mínimo (M1a).
--
-- Escopo desta migration:
--   * entidades do domínio musical (artists, albums, tracks, track_artists, artworks);
--   * arquivos de áudio indexados (audio_files), sem bytes de áudio no banco;
--   * índice de busca full-text (FTS5) mantido pela aplicação;
--   * auditoria de varreduras (scan_runs) e problemas encontrados (scan_issues).
--
-- Regras:
--   * A biblioteca é somente Opus (ADR-0007). O banco guarda o codec real encontrado
--     para auditoria, mas o scanner só indexa Opus.
--   * Nada é apagado automaticamente: arquivo ausente vira `available = 0`.
--   * IDs são UUIDv7 em TEXT (ADR-0009), independentes de caminho.

CREATE TABLE artists (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    normalized_name TEXT NOT NULL UNIQUE,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE albums (
    id              TEXT PRIMARY KEY NOT NULL,
    title           TEXT NOT NULL,
    normalized_title TEXT NOT NULL,
    album_artist_id TEXT REFERENCES artists (id) ON DELETE SET NULL,
    release_year    INTEGER,
    artwork_id      TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (album_artist_id, normalized_title)
);

CREATE TABLE tracks (
    id               TEXT PRIMARY KEY NOT NULL,
    album_id         TEXT REFERENCES albums (id) ON DELETE SET NULL,
    title            TEXT NOT NULL,
    normalized_title TEXT NOT NULL,
    disc_no          INTEGER,
    track_no         INTEGER,
    duration_ms      INTEGER,
    added_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE track_artists (
    track_id   TEXT NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    artist_id  TEXT NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    role       TEXT NOT NULL DEFAULT 'performer',
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (track_id, artist_id, role)
);

CREATE TABLE artworks (
    id            TEXT PRIMARY KEY NOT NULL,
    relative_path TEXT NOT NULL UNIQUE,
    source        TEXT NOT NULL,
    mime_type     TEXT,
    width         INTEGER,
    height        INTEGER,
    checksum      TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE audio_files (
    id             TEXT PRIMARY KEY NOT NULL,
    track_id       TEXT NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    storage_id     TEXT NOT NULL DEFAULT 'local',
    relative_path  TEXT NOT NULL UNIQUE,
    sha256         TEXT NOT NULL,
    codec          TEXT NOT NULL,
    container      TEXT NOT NULL,
    bitrate        INTEGER,
    sample_rate    INTEGER,
    channels       INTEGER,
    size_bytes     INTEGER NOT NULL,
    source_mtime   INTEGER NOT NULL,
    available      INTEGER NOT NULL DEFAULT 1 CHECK (available IN (0, 1)),
    original_file_id TEXT REFERENCES audio_files (id) ON DELETE SET NULL,
    scanned_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_tracks_album ON tracks (album_id);
CREATE INDEX idx_tracks_normalized_title ON tracks (normalized_title);
CREATE INDEX idx_albums_album_artist ON albums (album_artist_id);
CREATE INDEX idx_albums_normalized_title ON albums (normalized_title);
CREATE INDEX idx_audio_files_track ON audio_files (track_id);
CREATE INDEX idx_audio_files_sha256 ON audio_files (sha256);
CREATE INDEX idx_audio_files_available ON audio_files (available);
CREATE INDEX idx_track_artists_artist ON track_artists (artist_id);

-- FTS5 mantido pela aplicação (insert/delete explícitos) porque o conteúdo é
-- derivado de três tabelas. `remove_diacritics 2` cobre busca em PT-BR.
CREATE VIRTUAL TABLE tracks_fts USING fts5 (
    track_id UNINDEXED,
    title,
    album,
    artists,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TABLE scan_runs (
    id              TEXT PRIMARY KEY NOT NULL,
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    state           TEXT NOT NULL CHECK (state IN ('running', 'completed', 'failed')),
    files_seen      INTEGER NOT NULL DEFAULT 0,
    files_indexed   INTEGER NOT NULL DEFAULT 0,
    files_unchanged INTEGER NOT NULL DEFAULT 0,
    files_skipped   INTEGER NOT NULL DEFAULT 0,
    files_failed    INTEGER NOT NULL DEFAULT 0,
    files_missing   INTEGER NOT NULL DEFAULT 0,
    error           TEXT
);

CREATE TABLE scan_issues (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_run_id   TEXT NOT NULL REFERENCES scan_runs (id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    code          TEXT NOT NULL,
    detail        TEXT
);

CREATE INDEX idx_scan_issues_run ON scan_issues (scan_run_id);

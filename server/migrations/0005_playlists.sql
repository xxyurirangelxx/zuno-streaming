-- Migration 0005 — playlists e favoritos (M2c).
--
-- Decisões:
--   * playlists pertencem a um único usuário (`owner_id`); compartilhamento
--     público fica fora do M2c, por isso `visibility` aceita apenas
--     `private`/`unlisted`;
--   * nome único por dono via `normalized_name` (case/espaços ignorados);
--   * itens guardam `position` explícita (0..n-1) e não podem repetir a faixa;
--   * favoritos são polimórficos por `(entity_kind, entity_id)` sem FK, porque
--     o alvo varia entre `tracks`, `albums` e `artists`;
--   * `version` é incrementada em toda mutação da playlist, junto de `updated_at`;
--   * IDs são UUIDv7 em TEXT (ADR-0009).

CREATE TABLE playlists (
  id TEXT PRIMARY KEY NOT NULL,
  owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  normalized_name TEXT NOT NULL,
  visibility TEXT NOT NULL CHECK (visibility IN ('private','unlisted')) DEFAULT 'private',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  version INTEGER NOT NULL DEFAULT 1
);
CREATE UNIQUE INDEX ux_playlists_owner_name ON playlists (owner_id, normalized_name);
CREATE TABLE playlist_items (
  id TEXT PRIMARY KEY NOT NULL,
  playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (playlist_id, track_id)
);
CREATE INDEX idx_playlist_items_order ON playlist_items (playlist_id, position);
CREATE TABLE favorites (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  entity_kind TEXT NOT NULL CHECK (entity_kind IN ('track','album','artist')),
  entity_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY (user_id, entity_kind, entity_id)
);

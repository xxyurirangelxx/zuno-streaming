-- Migration 0006 — preferências, privacidade e histórico de audição (M2e).
--
-- Decisões:
--   * uma linha por usuário em `user_preferences` e `user_privacy` (PK = user_id),
--     criada sob demanda pelo backend com os defaults do produto;
--   * `theme` restrito a light/dark/system e `language` livre (validado na API);
--   * `history_retention_days` nulo significa "sem expiração"; quando definido,
--     aceita 1..3650 dias (validado na API e no CHECK);
--   * `play_events` é append-only e pertence sempre a um usuário (o cliente nunca
--     escolhe `user_id`); apagar o usuário ou a faixa remove os eventos em cascata;
--   * `seconds` é opcional porque `skip` pode não informar posição;
--   * o índice `(user_id, occurred_at DESC, id DESC)` serve a listagem keyset.

CREATE TABLE user_preferences (
  user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  theme TEXT NOT NULL CHECK (theme IN ('light','dark','system')) DEFAULT 'system',
  language TEXT NOT NULL DEFAULT 'pt-BR',
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE user_privacy (
  user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  allow_cloud_history INTEGER NOT NULL DEFAULT 1 CHECK (allow_cloud_history IN (0,1)),
  history_retention_days INTEGER CHECK (
    history_retention_days IS NULL OR history_retention_days BETWEEN 1 AND 3650
  ),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE play_events (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  event_kind TEXT NOT NULL CHECK (event_kind IN ('play','skip','complete')),
  seconds REAL,
  occurred_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX idx_play_events_user_time ON play_events (user_id, occurred_at DESC, id DESC);

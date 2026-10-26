-- Migration 0004 — usuários, sessões, convites e auditoria (M2a).
--
-- Decisões:
--   * papel em coluna (admin/moderator/user) com permissões no código, em vez de
--     tabela `roles` — o MVP tem três papéis fixos (ver ADR-0010);
--   * senhas com Argon2id (hash PHC completo em `password_hash`);
--   * sessões guardam apenas o sha256 do token (o token em claro nunca é persistido);
--   * convites e código de setup também são persistidos como hash;
--   * histórico de auditoria administrativa separado do histórico de audição.

CREATE TABLE users (
    id                  TEXT PRIMARY KEY NOT NULL,
    username            TEXT NOT NULL,
    normalized_username TEXT NOT NULL UNIQUE,
    password_hash       TEXT NOT NULL,
    role                TEXT NOT NULL CHECK (role IN ('admin', 'moderator', 'user')),
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    disabled_at         TEXT
);

CREATE TABLE sessions (
    id           TEXT PRIMARY KEY NOT NULL,
    user_id      TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash   TEXT NOT NULL UNIQUE,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    expires_at   TEXT NOT NULL,
    revoked_at   TEXT,
    last_seen_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_sessions_user ON sessions (user_id);
CREATE INDEX idx_sessions_expiry ON sessions (expires_at);

CREATE TABLE setup_tokens (
    id         TEXT PRIMARY KEY NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    expires_at TEXT NOT NULL,
    used_at    TEXT
);

CREATE TABLE invitations (
    id         TEXT PRIMARY KEY NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    role       TEXT NOT NULL CHECK (role IN ('admin', 'moderator', 'user')),
    created_by TEXT REFERENCES users (id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    expires_at TEXT NOT NULL,
    redeemed_at TEXT,
    revoked_at  TEXT
);

CREATE TABLE audit_events (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    actor_id  TEXT REFERENCES users (id) ON DELETE SET NULL,
    action    TEXT NOT NULL,
    target    TEXT,
    at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    detail    TEXT
);

CREATE INDEX idx_audit_actor ON audit_events (actor_id);
CREATE INDEX idx_audit_at ON audit_events (at);

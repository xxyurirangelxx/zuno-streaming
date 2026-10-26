-- Migration 0003 — uploads com staging e staging auditável (M1c).
--
-- O upload chega em `ZUNO_STAGING_DIR`, é validado (somente Opus, ADR-0007),
-- recebe sha256 e só então é movido atomicamente para a biblioteca. Nenhum
-- arquivo do usuário é sobrescrito ou apagado: duplicata exata responde 409 e
-- o arquivo existente permanece intacto.

CREATE TABLE uploads (
    id                TEXT PRIMARY KEY NOT NULL,
    original_filename TEXT NOT NULL,
    staging_path      TEXT NOT NULL,
    size_bytes        INTEGER NOT NULL DEFAULT 0,
    sha256            TEXT,
    status            TEXT NOT NULL CHECK (status IN ('staged', 'indexed', 'duplicate', 'failed')),
    track_id          TEXT REFERENCES tracks (id) ON DELETE SET NULL,
    error             TEXT,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at      TEXT
);

CREATE INDEX idx_uploads_status ON uploads (status);
CREATE INDEX idx_uploads_sha256 ON uploads (sha256);

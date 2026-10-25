-- Migration de bootstrap do M0.
--
-- Esta tabela guarda apenas metadados operacionais do servidor (versão do
-- schema, marcas de manutenção, entre outros). O schema de domínio musical
-- (tracks, artistas, álbuns, arquivos de áudio, artwork etc.) será introduzido
-- no marco M1, com suas próprias migrations.

CREATE TABLE IF NOT EXISTS zuno_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT OR IGNORE INTO zuno_meta (key, value) VALUES ('schema_baseline', 'm0');

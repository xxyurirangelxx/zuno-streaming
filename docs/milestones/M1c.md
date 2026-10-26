# Marco M1c — Upload com staging

Terceira e última entrega do M1. Sem autenticação (ADR-0008): loopback apenas.

## Escopo entregue

| Endpoint | Função |
|---|---|
| `POST /api/v1/uploads` | Recebe corpo binário + `X-Zuno-Filename`; grava em staging, calcula sha256, valida Opus |
| `GET /api/v1/uploads/{id}` | Status (`staged`/`indexed`/`duplicate`/`failed`), tamanho, sha256, `track_id` |
| `POST /api/v1/uploads/{id}/complete` | Move atomicamente para a biblioteca e indexa na hora |
| `DELETE /api/v1/uploads/{id}` | Descarta **somente** o arquivo em staging |

- **Somente Opus** (ADR-0007): não-Opus → `415 unsupported_format`; Opus corrompido → `422 invalid_audio`.
- **Limite** `ZUNO_MAX_AUDIO_FILE_BYTES` aplicado durante a leitura → `413 too_large` (staging removido).
- **Staging fora da biblioteca** (`ZUNO_STAGING_DIR`); o serviço recusa iniciar se o staging estiver dentro de `ZUNO_MUSIC_DIR`.
- Nome do arquivo enviado é **apenas metadado**; o arquivo em staging usa o UUID do upload.
- Destino derivado das tags: `Artista/Álbum/01 Título.opus` (componentes sanitizados; nunca traversal).
- **Duplicata exata** (mesmo sha256) → `409 duplicate` com o `track_id` existente; nada é sobrescrito ou apagado.
- Colisão de nome com conteúdo diferente → sufixo ` (2)`, ` (3)`…
- `EXDEV` tratado com cópia para `.tmp` + `rename` no destino; falha após o move marca `failed` e preserva o arquivo.
- Limpeza best-effort de staging órfão (> 24 h) na inicialização.
- Migration `0003_uploads.sql` mantém o histórico dos uploads.

## Evidências de execução

Windows 10, Rust 1.99 (MSVC), 2026-10-26.

```text
$ cargo fmt --all --check                 -> exit 0
$ cargo clippy -p zuno-server --all-targets -- -D warnings -> 0 warnings
$ cargo test -p zuno-server               -> 104 passed; 0 failed
  lib 45 | main 3 | api_library 10 | artwork 3 | health 3 | metadata 7
  migrations 3 | repository 13 | scanner 6 | streaming 6 | uploads 5
```

Smoke test com o binário real (porta 18092):

```text
POST /api/v1/uploads (tone-a.opus)  -> 201 staged, 30.873 bytes
GET  /api/v1/uploads/{id}           -> staged
POST /api/v1/uploads/{id}/complete  -> 200 indexed, track_id ...
GET  /api/v1/tracks                 -> [("Tone A", available=true)]
arquivo final                       -> Zuno Test/Fixture Album/01 Tone A.opus
staging                             -> vazio
segundo upload do mesmo conteúdo    -> 409 duplicate (mesmo track_id)
biblioteca após a duplicata         -> 1 arquivo (nada sobrescrito)
vorbis.ogg                          -> 415 unsupported_format
```

## Fora do M1c

- Autenticação/autorização do upload (M2).
- Upload resumível em blocos (`/uploads/{id}` com chunks) — o PRD prevê; fica para depois do MVP.
- Revisão interativa de duplicatas na UI (M2).
- Medição no Pi 3 B.

## Gate do M1

Com M1a + M1b + M1c, resta apenas a **medição em hardware** (Pi 3 B, armv7), pendente de disponibilidade.
Até lá o gate é declarado parcial e documentado como tal.

# Marco M1b — Streaming, API de biblioteca e capas

Segunda entrega do M1. Sem autenticação (ADR-0008): o servidor escuta em loopback e não deve ser exposto.

## Escopo entregue

### Streaming (`GET|HEAD /api/v1/tracks/{id}/stream`)

- `200` completo, `206` parcial e `416` com `Content-Range: bytes */tamanho`.
- Intervalos suportados: `bytes=a-b`, `bytes=a-`, `bytes=-n`; múltiplos intervalos devolvem a representação completa (documentado, sem multipart).
- `ETag` = sha256 do arquivo; `If-Range` divergente devolve `200` completo.
- `Accept-Ranges: bytes`, `Content-Type: audio/ogg` (biblioteca somente Opus), `Cache-Control: private`.
- `HEAD` com os mesmos cabeçalhos e corpo vazio.
- Caminho resolvido com `canonicalize` + verificação de prefixo da raiz: nunca serve fora de `ZUNO_MUSIC_DIR`.
- Faixa sem arquivo disponível ou arquivo removido do disco → `404` (sem vazar caminhos).

### API de biblioteca

| Endpoint | Função |
|---|---|
| `GET /api/v1/tracks` | Lista com `limit`, `cursor`, `album_id`, `artist_id`, `q` |
| `GET /api/v1/tracks/{id}` | Detalhe + arquivo de áudio (preferindo `available = 1`) |
| `GET /api/v1/albums` | Lista com `track_count` e artista do álbum |
| `GET /api/v1/artists` | Lista com `track_count`/`album_count` |
| `GET /api/v1/search` | Faixas via FTS5 (sem acento) + álbuns/artistas por prefixo |
| `GET /api/v1/artworks/{id}` | Capa original ou variante redimensionada em cache |

- Paginação **keyset** com cursor opaco (hex de `chave:id`), sem `OFFSET`; `limit` 1–200 (padrão 50).
- Sem N+1: uma consulta por página + uma consulta `IN (...)` para artistas das faixas.
- Cursor inválido → `400 invalid_cursor`; `size` inválido em capas → `400 invalid_query`.

### Capas

- Original servido com o MIME real (detectado por magic bytes quando ausente).
- Variantes `thumb` (320), `card` (640) ou tamanho numérico (16–2048) em JPEG qualidade 82.
- Cache imutável em `ZUNO_ARTWORK_DIR/derived/{artwork_id}-{size}.jpg`, escrita atômica.

### Contrato

`docs/api/openapi.yaml` atualizado (versão 0.2.0) com todos os endpoints, schemas e respostas de erro.

## Evidências de execução

Windows 10, Rust 1.99 (MSVC), 2026-10-25.

```text
$ cargo fmt --all --check
exit 0

$ cargo clippy -p zuno-server --all-targets -- -D warnings
Finished (0 warnings)

$ cargo test -p zuno-server
lib 31 | main 3 | api_library 10 | artwork 3 | health 3 | metadata 7
migrations 3 | repository 13 | scanner 6 | streaming 6
total 85 passed; 0 failed
```

Smoke test com o binário real (biblioteca com 2 Opus, porta 18090):

```text
GET /api/v1/tracks          -> 2 faixas, ambas available, 1 com artwork
GET /api/v1/search?q=tone   -> 2 faixas
GET .../stream              -> 200 audio/ogg 30.873 bytes
GET .../stream (Range 0-99) -> 206 content-range: bytes 0-99/30873, 100 bytes
GET .../stream (Range 30873-) -> 416 {"code":"range_not_satisfiable", ...}
GET /artworks/{id}?size=thumb -> 200 image/jpeg 7.419 bytes + cache {id}-320.jpg
```

## Fora do M1b

- Upload/staging e `415` para não-Opus (M1c).
- Autenticação/autorização (M2, ADR-0008).
- `cover.jpg`/`folder.jpg` como fonte de capa (hoje só embutida).
- Busca de álbuns/artistas sem acento (só faixas via FTS).

## Pendências

- Gate do M1 depende de M1c e da medição em Pi 3 B (hardware pendente).
- Rate limiting e cache HTTP mais agressivo ficam para o M2 (com auth).

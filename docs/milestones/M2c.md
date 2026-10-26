# Marco M2c — Playlists e favoritos

Terceira entrega do M2. Backend e frontend entregues juntos, com isolamento por usuário.

## Escopo entregue

### Playlists

- Migration `0005`: `playlists` (dono, nome normalizado único por usuário, visibilidade `private`/`unlisted`,
  `version`) e `playlist_items` (posição, único por faixa).
- `GET/POST /api/v1/playlists`, `GET/PATCH/DELETE /api/v1/playlists/{id}`.
- `POST/DELETE` de itens e `PUT .../position` (0-based) com recompactação e reordenação transacionais.
- Ownership estrito: playlist de outro usuário responde **404** (não revela existência).
- Nome duplicado (case/espaços normalizados) → `409`; faixa duplicada → `409`.
- `updated_at`/`version` atualizados em toda mutação.

### Favoritos

- Tabela `favorites` (`user_id`, `entity_kind` track/album/artist, `entity_id`), idempotente.
- `GET /api/v1/favorites` devolve faixas, álbuns e artistas agrupados.
- `PUT|DELETE /api/v1/favorites/{kind}/{id}` → `204` idempotente; entidade inexistente → `404`.

### Interface

- Navegação “Playlists” e “Favoritos”.
- Lista de playlists com criar/renomear/excluir (confirmação explícita) e contagem de faixas.
- Detalhe da playlist: tocar tudo (fila do player existente), remover faixa, mover para cima/baixo com
  atualização otimista e rollback.
- “Adicionar à playlist” no menu da faixa, com seletor, criação inline e feedback de duplicada.
- Coração em faixas, álbuns e artistas com `FavoritesProvider` (PUT/DELETE otimista + rollback).
- Diálogos acessíveis (`role="dialog"`, foco, Tab, Esc, restauração de foco), `aria-live`, `motion-safe`.

## Evidências de execução

Windows 10, Rust 1.99 / Node 24, 2026-10-26.

```text
$ cargo fmt --all --check                                  -> exit 0
$ cargo clippy -p zuno-server --all-targets -- -D warnings  -> 0 warnings
$ cargo test -p zuno-server                                -> 115 passed; 0 failed
$ npm run typecheck / lint                                 -> OK, 0 warnings
$ npm test                                                 -> 67 testes (44 web + 23 api-client)
$ npm run build                                            -> JS 301,11 kB (gzip 87,63 kB)
$ npx prettier --check .                                   -> OK
```

Smoke test real (porta 18096):

```text
POST /playlists {"name":"Minha Mix"}      -> 201
POST /playlists (nome duplicado)          -> 409
POST /playlists/{id}/items (2 faixas)     -> 201 {"position":0} / {"position":1}
POST .../items (duplicada)                -> 409
GET  /playlists/{id}                      -> track_count=2, ordem [Tone A, Tone B]
PUT  .../items/{T2}/position {0}          -> 204, ordem [Tone B, Tone A]
PUT  /favorites/track/{id}                -> 204 (repetir: 204 idempotente)
GET  /favorites                           -> tracks: ["Tone A"]
```

## Fora do M2c

- Compartilhamento de playlist (`unlisted` já existe no schema, sem UI de link público).
- Colaboração/multiusuário na mesma playlist.
- Histórico de audição, privacidade e preferências (M2e).
- Upload na interface (M2d).

## Pendências

- Nenhuma migração de dados necessária (tabelas novas).
- E2E em navegador real continua pendente (M2e).

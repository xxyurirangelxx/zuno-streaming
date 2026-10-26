# Marco M2e — Preferências, privacidade e histórico

Última entrega funcional do M2. Fecha o fluxo “usuário configura e vê só os próprios dados”.

## Escopo entregue

### Backend (migration 0006)

- `user_preferences` (tema light/dark/system, idioma), `user_privacy` (consentimento de nuvem, retenção),
  `play_events` (play/skip/complete + segundos) com índice keyset.
- `GET/PUT /api/v1/me/preferences`, `GET/PUT /api/v1/me/privacy`.
- `POST/GET/DELETE /api/v1/me/history` com paginação por cursor (`occurred_at DESC, id DESC`).
- `user_id` sempre do `CurrentUser`; histórico e preferências nunca acessíveis a terceiros.
- Validação: tema, idioma (2–10), retenção 1–3650 ou nula, `event_kind`, segundos 0–86400, faixa existente.

### Frontend

- `ThemeProvider`: aplica `data-theme` em `<html>`, carrega do servidor após login, persistência otimista com rollback;
  “sistema” remove o atributo (tokens usam `prefers-color-scheme`).
- **Configurações**: tema acessível, idioma `pt-BR` (EN desabilitado, “em breve”), aviso de que
  reduced-motion/transparency/contraste seguem o sistema.
- **Privacidade**: consentimento de histórico em nuvem + retenção (nunca/30/90/365), com texto de que recursos
  locais continuam e que a IA chega no M7.
- **Histórico**: lista com capa/título/artista, tipo de evento, data/hora, paginação e “Limpar histórico”
  com confirmação acessível.
- **Player**: registra `play` ao iniciar e `complete` ao terminar (no máximo um play por reprodução), falhas silenciosas.

## Evidências de execução

Windows 10, Rust 1.99 / Node 24, 2026-10-26.

```text
$ cargo fmt --all --check                                  -> exit 0
$ cargo clippy -p zuno-server --all-targets -- -D warnings  -> 0 warnings
$ cargo test -p zuno-server                                -> 125 passed; 0 failed
$ npm run typecheck / lint                                 -> OK, 0 warnings
$ npm test                                                 -> 97 testes (65 web + 32 api-client)
$ npm run build                                            -> JS 332,67 kB (gzip 95,10 kB)
$ npx prettier --check .                                   -> OK
```

Smoke test real (porta 18097/18098):

```text
GET  /me/preferences           -> {"theme":"system","language":"pt-BR"}
PUT  /me/preferences {dark}    -> {"theme":"dark",...}
PUT  /me/preferences {neon}    -> 400
GET  /me/privacy               -> {"allow_cloud_history":true,"history_retention_days":null}
PUT  /me/privacy {false,90}    -> {"allow_cloud_history":false,"history_retention_days":90}
POST /me/history play/complete -> 201
GET  /me/history?limit=1       -> 1 item + next_cursor
GET  /me/history               -> 2 itens (complete, play)
DELETE /me/history             -> 204, lista vazia
```

## Limitações conhecidas

- `allow_cloud_history` e `history_retention_days` estão **persistidos**, mas só serão aplicados no M7
  (envio para provedores de IA e job de expurgo). Nenhum comportamento foi inventado.
- E2E automatizado em navegador real (Playwright) não foi executado; a validação foi por API + testes de componente.
- Idioma inglês ainda não traduzido (infraestrutura i18n planejada para o M8).

## Gate do M2

Fluxo funcional completo: setup do admin → convite → login → biblioteca → player → playlist → favoritos →
upload → preferências/privacidade/histórico, com isolamento entre usuários testado (404/401/403).
Resta apenas o E2E em navegador real para automatizar esse fluxo ponta a ponta.

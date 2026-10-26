# Marco M2b — Web: sessão, biblioteca, busca e player

Segunda entrega do M2. O frontend deixa de ser a página de conectividade do M0 e passa a ser o shell do app.

## Escopo entregue

### Sessão e onboarding

- Bootstrap: `GET /api/v1/me` → 401 → `GET /api/v1/setup/status` → tela de **setup** (código + usuário + senha)
  ou **login**; 401 durante o uso volta ao login sem loop.
- Resgate de convite (`/api/v1/invitations/redeem`) na tela de entrada.
- Logout no menu do usuário; cookie `HttpOnly` (nenhum token em `localStorage`/`sessionStorage`).

### Shell e navegação

- Sidebar no desktop, navegação inferior no mobile; Início, Biblioteca (faixas/álbuns/artistas), Buscar.
- Router leve por `hashchange` (sem dependência nova).
- Tokens de design existentes, vidro moderado, `motion-safe`, foco visível, contraste AA,
  `prefers-reduced-motion`/`transparency`, ícones SVG inline.

### Biblioteca e busca

- Faixas com artistas, álbum, duração e capa (`/api/v1/artworks/{id}?size=thumb`).
- Álbuns e artistas com contadores; paginação “Carregar mais” por cursor.
- Estados de carregamento (skeleton), vazio (“biblioteca vazia — envie arquivos Opus”) e erro acionável.
- Busca com debounce de 300 ms, seções faixas/álbuns/artistas, limpeza imediata e `role="search"`.

### Player

- `HTMLAudioElement` único com stream autenticado por cookie (mesma origem via proxy `/api`).
- Play/pause, anterior/próximo, seek com tempo atual/total, volume/mudo, shuffle (sem repetir até esgotar),
  repeat off/all/one, fila com remover/subir/descer.
- Atalhos: espaço (play/pause), ←/→ (seek 5 s), Shift+←/→ (faixa anterior/próxima).
- Media Session API best-effort (metadados/handlers) com fallback silencioso.
- Erro de reprodução vira aviso no player, sem quebrar a interface.

### API client

- Métodos tipados para auth/setup/convites/biblioteca/mídia; `credentials: 'include'`;
  `ZunoApiError` com `code`/`status` e `isUnauthorizedError`.
- Tipos compartilhados espelhando o OpenAPI 0.3.0.
- Proxy do Vite: `/api` e `/health` → `ZUNO_DEV_API_TARGET` (padrão `127.0.0.1:8080`).

## Evidências de execução

Windows 10, Node 24 / npm 12, 2026-10-26.

```text
$ npm run typecheck        -> OK (web, api-client, shared-types)
$ npm run lint             -> 0 erros, 0 warnings
$ npm test                 -> web 31 testes (5 arquivos) + api-client 17 testes
$ npm run build            -> 39 módulos; JS 274,22 kB (gzip 82,48 kB); CSS 31,01 kB
$ npx prettier --check .   -> All matched files use Prettier code style!
```

Smoke test real (backend + Vite na mesma origem, porta 5175):

```text
GET  /api/v1/setup/status (proxy)   -> {"required":true}
POST /api/v1/setup/claim (proxy)    -> 201 admin + Set-Cookie
GET  /api/v1/me (cookie via proxy)  -> admin
GET  /api/v1/tracks (cookie)        -> {"items":[],"next_cursor":null}
GET  /                              -> <title>Zuno — Streaming de Músicas</title>
```

## Fora do M2b

- Upload na interface e revisão de duplicatas (M2c).
- Playlists, favoritos e compartilhamento (M2d).
- Preferências, privacidade e histórico individual (M2e).
- Testes E2E em navegador real (Playwright) e matriz de codecs (M2e).
- Tema claro/escuro explícito na UI (tokens já suportam; seletor fica no M2e).

## Pendências

- 401 no stream de áudio aparece como erro no player (o `HTMLAudioElement` não expõe o status); melhoria futura
  com pré-checagem `HEAD`.
- Media Session depende do navegador.

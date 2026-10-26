# Marco M2a — Autenticação, RBAC e convites

Primeira entrega do M2. A partir daqui **nenhuma rota de biblioteca, mídia ou upload é pública**
(ADR-0010); o M1 havia sido entregue sem auth por decisão temporária (ADR-0008).

## Escopo entregue

### Contas e sessões

- Argon2id (hash PHC em `users.password_hash`), senha mínima de 8 caracteres.
- Sessão por cookie `zuno_session` (`HttpOnly`, `SameSite=Lax`, 30 dias, `Secure` quando `ZUNO_COOKIE_SECURE=true`).
- Banco guarda apenas `sha256(token)`; token em claro só existe no cookie.
- `POST /api/v1/auth/refresh` rotaciona o token e revoga o anterior.
- `POST /api/v1/auth/logout` revoga e limpa o cookie.
- Troca de senha/desabilitação revoga todas as sessões (`revoke_user_sessions`).

### Setup inicial e convites

- Sem usuários, o servidor gera um código de 8 caracteres (sem `0/O/I`), guarda o hash (24 h) e imprime **uma vez** no log.
- `GET /api/v1/setup/status` e `POST /api/v1/setup/claim` (uso único, cria o admin e a sessão).
- Convites com token de 256 bits (hash no banco), validade 1–30 dias (padrão 7), uso único e revogáveis.
- Apenas admin cria/lista/revoga convites; `role: admin` é recusado no convite.
- `POST /api/v1/invitations/redeem` é público e cria usuário + sessão.

### RBAC e proteção

- Papéis `admin`, `moderator`, `user` (defaults restritivos).
- Middleware `require_auth` injeta `CurrentUser`; rotas públicas: `health`, `setup/*`, `auth/login`, `invitations/redeem`.
- `same_origin_guard` rejeita mutações com `Origin` divergente ou `Sec-Fetch-Site: cross-site`.
- Rate limit de login em memória: 5 tentativas por `usuário` a cada 5 minutos → `429`.
- `audit_events` registra setup, criação/revogação de convite e redenção.

### Contrato

`docs/api/openapi.yaml` 0.3.0: `securitySchemes.cookieAuth`, rotas de auth/setup/convites, overrides
`security: []` nas rotas públicas e respostas 401/403/409/429.

## Evidências de execução

Windows 10, Rust 1.99 (MSVC), 2026-10-26.

```text
$ cargo fmt --all --check                                  -> exit 0
$ cargo clippy -p zuno-server --all-targets -- -D warnings  -> 0 warnings
$ cargo test -p zuno-server                                -> 110 passed; 0 failed
  lib 45 | main 3 | api_library 10 | artwork 3 | auth 6 | health 3 | metadata 7
  migrations 3 | repository 13 | scanner 6 | streaming 6 | uploads 5
```

Smoke test com o binário real (porta 18094):

```text
log de inicialização        -> setup_code=W3FUCFCZ (uma vez, hash no banco)
GET  /api/v1/setup/status   -> {"required":true}
GET  /api/v1/tracks (sem)   -> 401 unauthorized
POST /api/v1/setup/claim    -> 201 {"role":"admin"} + Set-Cookie
GET  /api/v1/me             -> admin
GET  /api/v1/tracks (com)   -> 200
POST /api/v1/invitations    -> token (uma vez), role user, 7 dias
POST /invitations/redeem    -> 201 {"role":"user"} + Set-Cookie
POST /auth/login            -> 200 + Set-Cookie
POST /invitations (ouvinte) -> 403 forbidden
POST /uploads (Origin evil) -> 403 (guarda de mesma origem)
```

## Segurança

- Nenhum token em claro no banco; cookies `HttpOnly` (imunes a leitura por XSS).
- Mensagem genérica em login (não revela se o usuário existe).
- Setup e convites são de uso único e expiram.
- CSRF mitigado por `SameSite=Lax` + verificação de origem.
- Auditoria administrativa sem dados sensíveis.

## Fora do M2a

- Passkeys/WebAuthn (RP ID pendente no PRD).
- Recuperação de senha por e-mail (e-mail opcional, sem SMTP configurado).
- Rate limit persistido entre reinícios (M4/multi-instância).
- Telas de login/setup/convite no frontend (M2b).
- Interface administrativa de usuários (M2e).

## Pendências

- Frontend ainda não consome a autenticação (M2b).
- Medição em Pi 3 B continua pendente.

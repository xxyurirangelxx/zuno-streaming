# ADR-0010 — Autenticação por sessão em cookie, Argon2id e papéis fixos

- **Status:** Accepted
- **Data:** 2026-10-26
- **Decisores:** proprietário do projeto
- **Fonte:** PRD §2.5, §7; PLANO_AGENTE M2

## Contexto

O M2 introduz contas, convites e RBAC. O PRD exige senhas com Argon2id, sessões renováveis e revogáveis,
convites expirados, papéis admin/moderador/usuário com defaults restritivos, limitação de tentativas e
proteção contra CSRF. A API é consumida por um SPA na mesma origem (via proxy do Vite) e, no futuro, por
clientes Tauri/Android.

Alternativas de transporte de credencial: cookie de sessão `HttpOnly` (não acessível a JavaScript) ou
token em `localStorage` (vulnerável a XSS e comum em SPAs). O PRD §5.4 pede explicitamente “cookies de sessão
protegidos ou mecanismo compatível sem token permanente em URL” para streaming em browser.

## Decisão

1. **Senhas:** Argon2id com os parâmetros padrão do crate `argon2`; hash PHC completo em `users.password_hash`.
   Senha mínima de 8 caracteres; mensagens de erro nunca revelam se o usuário existe.
2. **Sessão:** token aleatório de 256 bits (dois UUIDv4), entregue em cookie
   `zuno_session` com `HttpOnly; SameSite=Lax; Path=/; Max-Age=30 dias` e `Secure` quando a instância
   estiver sob HTTPS. O banco guarda **apenas** `sha256(token)`.
3. **Renovação:** `POST /api/v1/auth/refresh` rotaciona o token (cria novo, revoga o anterior) mantendo a
   mesma sessão lógica; `POST /api/v1/auth/logout` revoga. Trocar senha ou desabilitar o usuário revoga
   todas as sessões.
4. **Papéis fixos** (`admin`, `moderator`, `user`) em coluna com `CHECK`, permissões no código
   (`Role::is_admin`, `Role::can_moderate`). Sem tabela `roles` no MVP: três papéis fixos não justificam
   joins e seeds; a migração para tabela fica registrada como possível no M8.
5. **Setup inicial:** se não houver usuários, o servidor gera um código de 8 caracteres (sem `0/O/I`),
   guarda o hash com validade de 24 h e **imprime o código uma única vez no log**. `POST /api/v1/setup/claim`
   consome o código (uso único) e cria o admin. Sem usuário, o setup é o único endpoint mutável sem sessão.
6. **Convites:** token aleatório de 256 bits, hash no banco, validade padrão de 7 dias, uso único,
   revogável. `POST /api/v1/invitations/redeem` é público (com o token) e cria usuário + sessão.
7. **CSRF:** além de `SameSite=Lax`, mutações exigem mesma origem quando o navegador envia `Origin`
   (comparação com `Host`) e rejeitam `Sec-Fetch-Site: cross-site`. Clientes sem `Origin` (curl, testes,
   Tauri) continuam funcionando.
8. **Limite de tentativas:** em memória, 5 tentativas por `usuário@IP` a cada 5 minutos; reset no sucesso.
   Persistência de rate limit entre reinícios fica para o M4 (multi-instância não existe).
9. **Autorização:** middleware injeta `CurrentUser`; todas as rotas `/api/v1/*` exigem sessão, exceto
   `setup/*`, `auth/login`, `invitations/redeem` e `health`. Endpoints administrativos checam `is_admin`.
10. **Auditoria:** `audit_events` registra ações administrativas (criação de convite, revogação, mudança de
    senha) com ator, ação, alvo e detalhe redigido — nunca senhas ou tokens.

## Alternativas consideradas

| Alternativa | Prós | Contras | Motivo da recusa |
|---|---|---|---|
| JWT em `localStorage` | stateless, simples no cliente | XSS rouba token; revogação exige lista negra; PRD pede cookie para stream | sessão em cookie é revogável e mais segura |
| Cookie + CSRF token por formulário | defesa dupla | mais estados e endpoints no MVP | `SameSite=Lax` + checagem de origem cobre o cenário atual |
| Tabela `roles`/`role_permissions` | flexível para papéis customizados | joins e seeds sem necessidade real | papéis fixos até existir requisito |
| Rate limit persistido no SQLite | sobrevive a reinício | escrita extra por tentativa | em memória basta para 2–5 usuários |
| Argon2 com parâmetros “tuned” para Pi 3 | menor custo de CPU | risco de enfraquecer hash | parâmetros padrão do crate; medir no Pi antes de ajustar |

## Consequências

- **Positivas:** credencial inacessível a JavaScript; revogação imediata; convites e setup de uso único;
  nenhum token em claro no banco; auditoria administrativa desde o início.
- **Negativas:** SPA precisa de `credentials: 'include'` (mesma origem, via proxy, não exige CORS);
  clientes nativos futuros precisam repassar o cookie; sem rate limit distribuído.
- **Operacionais:** o código de setup aparece no log — documentado como segredo temporário; o administrador
  deve reivindicar imediatamente após a instalação.
- **Testes:** toda a suíte HTTP passa a autenticar; testes de negação (401/403/404) e de isolamento entre
  usuários são obrigatórios no M2.

## Status e próximos passos

- **Vigente a partir do M2.** Revisar quando existir: passkeys (RP ID), múltiplas instâncias (rate limit
  compartilhado), ou papéis customizados.

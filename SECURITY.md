# Política de segurança

O Zuno é um servidor **self-hosted** em desenvolvimento inicial. O M0 entrega apenas fundação, healthcheck e banco local; não há autenticação, streaming ou upload implementados. Trate esta versão como experimental.

## Versões suportadas

| Versão | Suporte |
|---|---|
| `0.1.x` (experimental, M0) | Correções de segurança enquanto o branch `main` estiver ativo |
| Versões futuras (M1+) | A definir quando houver releases |

Não há garantia de suporte para versões antigas, forks ou builds modificados por terceiros.

## Como reportar uma vulnerabilidade

Use o canal **privado** do GitHub:

1. Acesse a aba **Security** do repositório.
2. Escolha **Report a vulnerability** (GitHub Security Advisories).
3. Descreva o problema com passos de reprodução, impacto e versão/commit afetado.

**Não abra issue pública, discussão ou PR público para vulnerabilidades não corrigidas.** Isso expõe usuários antes de existir correção.

Se o canal privado estiver indisponível, abra uma issue **sem detalhes técnicos** pedindo um canal seguro de contato.

## Expectativas de resposta

- Não há SLA nem prazo garantido; o projeto é mantido em tempo parcial.
- Buscamos confirmar o recebimento e classificar o impacto assim que possível.
- Divulgação coordenada: combine o prazo com o mantenedor antes de publicar detalhes.
- Crédito público é opcional e combinado com quem reporta.

## Escopo

Aceitamos relatos sobre:

- Vazamento de segredos, credenciais ou dados pessoais.
- Bypass de bind local/exposição não intencional.
- Injeção (SQL, path traversal, header injection) e falhas de validação.
- Falhas em migrations, corrupção de banco ou perda de dados.
- Vulnerabilidades em dependências que afetem o Zuno.
- Manipulação de arquivos de música, capas ou backups (quando implementados).

Fora de escopo:

- Configurações deliberadamente inseguras feitas pelo operador (ex.: expor o servidor sem TLS/auth).
- Engenharia social, phishing e ataques físicos.
- Falhas em serviços de terceiros não controlados pelo projeto.
- Ausência de recursos ainda não implementados (auth, RBAC, upload) — isso é roadmap, não vulnerabilidade.

## O que nunca enviar

Não inclua em issues, PRs, advisories públicos ou mensagens:

- Chaves, tokens, senhas, cookies de sessão ou arquivos `.env`.
- Dumps de banco, backups, logs completos ou arquivos de música (direitos autorais e dados pessoais).
- Caminhos absolutos que revelem nomes de usuário ou estrutura privada do host.
- Dados de outras pessoas.

Redija/anonimize antes de enviar. Se precisar compartilhar um artefato, combine um canal privado.

## Limites de privacidade

- O Zuno não usa telemetria automática e não envia dados de uso para terceiros.
- Histórico de audição é privado por design (a partir do M2), inclusive de telas administrativas comuns.
- **Administrar o host não é proteção criptográfica.** Quem controla o sistema operacional, o filesystem ou o banco pode ler os dados. O Zuno não promete sigilo contra o dono do servidor/host; backups remotos devem ser criptografados.
- Integrações de IA ou metadados são opcionais; o envio do histórico à nuvem é veto por usuário (M7). Nenhuma integração é necessária para o núcleo funcionar.

## Controles já implementados no M0

- Bind padrão em `127.0.0.1:8080` — não exposto publicamente por padrão.
- Nenhum segredo no repositório; configuração apenas por variáveis de ambiente (`.env.example` como referência).
- Migrations embutidas e versionadas; sem schema ad hoc em runtime.
- SQLite com WAL e `foreign_keys=ON`; pool pequeno.
- Auditoria de dependências no CI (`cargo audit`, `npm audit --audit-level=high`).
- Sem telemetria, analytics ou SDKs de rastreamento.
- `unsafe_code = "forbid"` no workspace Rust; clippy com warnings como erros.

### Advisories de dependências

O `cargo audit` é executado em CI e localmente. Exceções são **explícitas** em [`.cargo/audit.toml`](../.cargo/audit.toml),
com justificativa técnica e critério de revisão. Situação atual:

| Advisory | Crate | Situação | Justificativa |
|---|---|---|---|
| RUSTSEC-2023-0071 | `rsa 0.9.10` | Ignorado com justificativa | Não está no grafo de build (`cargo tree -i rsa` não retorna nada); aparece só no `Cargo.lock` por dependência opcional do driver MySQL do SQLx. O Zuno usa apenas SQLite. Sem versão corrigida publicada. |

Não adicione novas exceções sem registrar o motivo, o impacto real e quando a decisão será revisada.

Detalhes de ameaças e mitigações planejadas por milestone: [`docs/security/threat-model.md`](docs/security/threat-model.md).

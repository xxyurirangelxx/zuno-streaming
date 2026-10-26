# Decisões arquiteturais (ADRs)

Registro das decisões relevantes do Zuno. Cada ADR é imutável depois de aceita: uma mudança de rumo gera uma nova ADR que substitui a anterior (`Superseded by ADR-XXXX`).

## Índice

| ID | Título | Status | Data |
|---|---|---|---|
| [0001](0001-backend-rust-monolito-modular.md) | Backend Rust com monólito modular | Accepted | 2026-10-25 |
| [0002](0002-sqlite-e-migrations.md) | SQLite com migrations embutidas | Accepted | 2026-10-25 |
| [0003](0003-arquitetura-de-clientes.md) | Arquitetura de clientes e capacidades | Accepted | 2026-10-25 |
| [0004](0004-armazenamento-de-audio.md) | Armazenamento de áudio no filesystem | Accepted | 2026-10-25 |
| [0005](0005-ferramentas-do-monorepo.md) | Ferramentas e workspaces do monorepo | Accepted | 2026-10-25 |
| [0006](0006-licenca-apache-2.0.md) | Licença Apache-2.0 | Accepted | 2026-10-25 |
| [0007](0007-opus-formato-canonico.md) | Opus 128 kbps VBR canônico e conversão no cliente | Accepted | 2026-10-25 |
| [0008](0008-m1-sem-autenticacao.md) | M1 sem autenticação (loopback, temporária) | Accepted (expira no M2) | 2026-10-25 |
| [0009](0009-dependencias-e-ids-m1.md) | Dependências do servidor musical e IDs UUIDv7 | Accepted | 2026-10-25 |

## Convenção de status

| Status | Significado |
|---|---|
| `Proposed` | Proposta registrada, aguardando decisão do proprietário. Não é vinculante. |
| `Accepted` | Decisão aprovada e vigente. |
| `Rejected` | Proposta recusada; mantida para histórico. |
| `Superseded` | Substituída por outra ADR (indicar o ID). |

## Template

Copie o bloco abaixo para `NNNN-titulo-curto.md` (numeração sequencial, quatro dígitos) e atualize o índice.

```markdown
# ADR-NNNN — Título

- **Status:** Proposed | Accepted | Rejected | Superseded by ADR-XXXX
- **Data:** AAAA-MM-DD
- **Decisores:** (quem decidiu)

## Contexto

Qual problema ou restrição exige uma decisão? Cite PRD/plano quando aplicável.

## Decisão

O que foi decidido, de forma objetiva.

## Alternativas consideradas

- Opção A — prós/contras e por que não foi escolhida.
- Opção B — idem.

## Consequências

O que muda, o que fica mais fácil, o que fica mais difícil, riscos e dívidas.

## Status e próximos passos

Estado atual, pendências e quando revisar.
```

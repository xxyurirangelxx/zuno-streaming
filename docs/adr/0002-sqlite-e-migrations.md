# ADR-0002 — SQLite com migrations embutidas

- **Status:** Accepted
- **Data:** 2026-10-25
- **Decisores:** proprietário do projeto (via PRD v2.0 e `PLANO_AGENTE.md`)
- **Fonte:** PRD §2.2, §5.1 e §5.3; PLANO_AGENTE M0

## Contexto

O servidor precisa de persistência transacional confiável em Raspberry Pi 3 (microSD, pouca RAM) e em Windows/Linux, sem servidor de banco separado. O esquema vai evoluir ao longo de M1–M8 e precisa de migrações reproduzíveis, testáveis e recuperáveis. Dados de usuário (playlists, histórico) e metadados da biblioteca exigem integridade referencial.

## Decisão

Usar **SQLite** como banco único do MVP:

- Driver **SQLx 0.8** (`runtime-tokio`, `sqlite`, `migrate`, `macros`), com pool pequeno por padrão (`ZUNO_DB_MAX_CONNECTIONS=4`, ajustável).
- **WAL** habilitado e `PRAGMA foreign_keys = ON` em toda conexão.
- **Migrations embutidas** no binário a partir de `server/migrations`, aplicadas na inicialização via `sqlx::migrate!`. Nada de schema ad hoc em runtime.
- Banco em arquivo (`sqlite://data/zuno.db` por padrão), criado automaticamente.
- **Sem PostgreSQL no MVP.** Não há requisito de múltiplos hosts de banco, replicação ou concorrência alta que justifique um servidor separado.

### Escopo do schema

- **M0:** baseline mínimo com a tabela `zuno_meta` (metadados de instância/esquema), apenas para provar o fluxo de migration e o healthcheck.
- **M1:** schema de domínio (tracks, artistas, álbuns, arquivos de áudio, capas, FTS5, jobs), com constraints, índices e análise de rollback.

## Alternativas consideradas

| Alternativa | Prós | Contras | Motivo da recusa |
|---|---|---|---|
| PostgreSQL | concorrência, recursos avançados | processo/serviço extra, RAM e instalação no Pi 3 | complexidade desnecessária no MVP |
| DuckDB | analítico | não é o perfil OLTP do app; ecossistema SQLx menos maduro | inadequado |
| Arquivos JSON/DIY | simples no início | sem transações/índices, corrupção sob concorrência | inaceitável para dados de usuário |
| SQLite sem migrations | rápido de escrever | evolução não reproduzível, risco de perda | viola requisito de migrations |

## Consequências

- Instalação sem dependência de servidor de banco; backup é copiar o arquivo (com cuidado de WAL) ou usar a API de backup futura.
- Escritas concorrentes são serializadas; manter transações curtas e pool pequeno.
- WAL gera `-wal`/`-shm`; o `.gitignore` já exclui esses arquivos e o diretório `data/`.
- Migrations destrutivas exigem backup e plano de rollback; restauração será testada em milestone próprio.
- Migrar para outro banco no futuro implicaria nova ADR.

## Próximos passos

- Definir índices e constraints do domínio no M1, incluindo FTS5.
- Testar migração do zero e reinício idempotente no CI.
- Especificar política de backup/restore consistente (M3).

# ADR-0001 — Backend Rust com monólito modular

- **Status:** Accepted
- **Data:** 2026-10-25
- **Decisores:** proprietário do projeto (via PRD v2.0 e `PLANO_AGENTE.md`)
- **Fonte:** PRD §2.2 e §5.1; PLANO_AGENTE §1.1 e §4

## Contexto

O Zuno precisa rodar em hardware modesto (Raspberry Pi 3), com 2–5 usuários e até 1.000 músicas, sem exigir serviços externos obrigatórios. Ao mesmo tempo, o domínio é amplo: biblioteca, streaming, autenticação, recomendações, sync e provedores opcionais. Microsserviços multiplicariam processos, memória e complexidade operacional sem necessidade comprovada nessa escala.

## Decisão

Adotar **um único executável Rust** como backend:

- Rust stable (edição 2024), **Axum 0.8** sobre **Tokio**.
- **Monólito modular**: módulos internos com fronteiras explícitas (config, API/HTTP, domínio, repositórios/SQLx, futuro `library`, `stream`, `sync`, `recommend`, `providers`, `jobs`).
- O domínio não depende de Axum, HTTP ou SQLx; repositórios encapsulam o acesso ao banco.
- Trabalho pesado (análise acústica, transcodificação, fingerprint) pode ser movido para **workers opcionais** externos, autenticados e com limites de recurso; o núcleo continua funcional sem eles.
- Logging estruturado com `tracing`; operações bloqueantes isoladas e concorrência limitada.

No M0 existe apenas `GET /health` e a base de configuração/pool; os módulos de domínio serão adicionados conforme o milestone.

## Alternativas consideradas

| Alternativa | Prós | Contras | Motivo da recusa |
|---|---|---|---|
| Microsserviços | isolamento, escala independente | memória/CPU, deploy e observabilidade complexos no Pi 3 | overengineering para 2–5 usuários |
| Node.js/TypeScript no backend | stack única | consumo de memória, binário único e baixo footprint pior | Rust é requisito confirmado |
| Go | binário único, simples | fora da stack definida; ecossistema de mídia/tipos escolhido é Rust | requisito do PRD |
| Monólito com workers obrigatórios | separação de carga | quebra instalação simples e uso offline | workers são opcionais |

## Consequências

- Binário único simplifica instalação, backup e `systemd`.
- Fronteiras modulares precisam de disciplina: erros de acoplamento não são barrados pelo compilador.
- Um processo com bug derruba toda a API; mitigado com testes, healthcheck e reinício supervisionado.
- Workers externos exigem autenticação e protocolo próprios quando forem implementados (M1+).

## Próximos passos

- Detalhar módulos de domínio no M1 mantendo as fronteiras.
- Documentar contratos internos e evitar dependência circular entre módulos.

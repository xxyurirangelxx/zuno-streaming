# Como contribuir com o Zuno

Obrigado pelo interesse. O Zuno é um projeto pequeno e incremental: mudanças focadas, testadas e documentadas são mais valiosas do que grandes alterações sem revisão.

> **Licença pendente.** Ainda não há licença de reutilização definida (não existe `LICENSE`; o projeto usa `UNLICENSED`). Enquanto a decisão não for registrada em [ADR-0006](docs/adr/0006-licenca-pendente.md), **não aceitamos contribuições externas significativas** — abra uma issue para discutir antes de investir tempo em código. Pequenas correções de documentação/typos podem ser propostas para discussão.

## Antes de começar

1. Leia [`PRD.md`](PRD.md) (requisitos, fonte de verdade) e [`PLANO_AGENTE.md`](PLANO_AGENTE.md) (milestones e ordem).
2. Verifique o milestone atual. O **M0 (fundação)** está concluído e o **M1 (servidor musical)** ainda não foi iniciado; não implemente funcionalidades de M1+ sem uma issue aceita.
3. Para o setup local, siga [`docs/installation/development.md`](docs/installation/development.md).

## Fluxo de contribuição

1. **Fork** do repositório (ou crie uma branch se você for mantenedor).
2. Crie uma branch com nome descritivo, por exemplo:
   - `docs/m0-threat-model`
   - `feat/m1-library-scanner`
   - `fix/health-degraded-status`
3. Faça mudanças pequenas e coesas, com testes.
4. Rode os checks locais (veja abaixo) e cole a saída real no PR.
5. Abra um **Pull Request** usando o template em [`.github/pull_request_template.md`](.github/pull_request_template.md).
6. Para propor trabalho, use o template de issue em [`.github/ISSUE_TEMPLATE/issue.md`](.github/ISSUE_TEMPLATE/issue.md). Use IDs estáveis do backlog (`ZUNO-001`, `ZUNO-002`, …) quando aplicável.

### Regras de PR

- Um PR = um milestone/issue. Evite misturar refatoração ampla com correção funcional.
- Não enfraqueça strictness (TypeScript strict, regras de ESLint, clippy `-D warnings`) para fazer o CI passar. Corrija o código.
- Não adicione dependências sem justificativa; prefira `std`/bibliotecas já usadas.
- Nada de serviços pagos, telemetria ou SDKs de rastreamento no núcleo.
- Mudanças de API exigem atualização de [`docs/api/openapi.yaml`](docs/api/openapi.yaml), tipos compartilhados e testes de contrato.
- Mudanças de schema exigem migration versionada e análise de rollback/restore.
- Mudanças arquiteturais relevantes exigem uma ADR em [`docs/adr/`](docs/adr/README.md).

## Convenção de commits

Use [Conventional Commits](https://www.conventionalcommits.org/):

```text
<tipo>(<escopo opcional>): <descrição no imperativo, minúscula, sem ponto final>

[corpo opcional]

[rodapé opcional: refs, BREAKING CHANGE]
```

Tipos aceitos:

| Tipo | Uso |
|---|---|
| `feat` | nova funcionalidade |
| `fix` | correção de bug |
| `docs` | documentação |
| `test` | testes |
| `refactor` | mudança interna sem alterar comportamento |
| `perf` | melhoria de desempenho |
| `build` | build, toolchain, dependências |
| `ci` | pipelines |
| `chore` | manutenção |

Exemplos:

```text
feat(server): add degraded database health response
docs(adr): record pending license decision
fix(web): handle health fetch failure without crashing
```

Mensagens em inglês são aceitas; a documentação e as mensagens de UI são em **PT-BR**.

## Checklist local

Execute a partir da raiz do repositório.

Backend:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Frontend:

```bash
npm ci
npm run format:check
npm run typecheck
npm run lint
npm run test
npm run build
```

Atalho equivalente (se disponível no seu sistema):

```bash
scripts/check.sh          # Linux/macOS
```

```powershell
scripts/check.ps1         # Windows PowerShell
```

> Cole no PR a saída real dos comandos que você executou. Não afirme ter rodado algo que não rodou; se um check não puder ser executado, explique o motivo.

## Estilo de código

- **Rust:** `rustfmt` (`cargo fmt`); `clippy` com `-D warnings`. `unsafe_code = "forbid"` no workspace. Identificadores em inglês, comentários e mensagens voltadas ao usuário em PT-BR quando fizer sentido.
- **TypeScript:** strict, sem `any` desnecessário. ESLint + Prettier. Componentes e hooks em inglês.
- **Tailwind:** use os design tokens (`packages/design-tokens`); evite cores mágicas.
- **Markdown:** links relativos corretos, blocos de código com linguagem, sem HTML desnecessário.
- **Mensagens de API:** erros seguem `{ "code": "snake_case", "message": "texto" }`.

## Segurança e privacidade

- **Nunca** comite segredos: tokens, chaves, senhas, `.env` real, dumps de banco ou metadados pessoais.
- O bind padrão é `127.0.0.1`. Não adicione exposição pública por padrão.
- Não adicione telemetria, analytics ou chamadas externas obrigatórias.
- **Nunca** delete músicas do usuário em código ou teste. Operações destrutivas exigem confirmação explícita.
- Reporte vulnerabilidades de forma privada, conforme [`SECURITY.md`](SECURITY.md). Não abra issue pública para falhas de segurança.

## Dúvidas

Abra uma issue de discussão usando o template existente. Decisões pendentes (licença, provedores externos, RP ID, hardware de referência) não devem ser tomadas unilateralmente por uma contribuição — registre a necessidade e aguarde o proprietário.

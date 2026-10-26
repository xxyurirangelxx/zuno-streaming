# Zuno

Zuno é uma plataforma **self-hosted** de streaming de músicas: você mantém sua própria biblioteca, no seu próprio hardware, e ouve de onde estiver. O servidor é escrito em Rust e foi dimensionado para rodar em um Raspberry Pi 3, enquanto os clientes (Web/PWA, Windows e Android) compartilham a mesma interface em React. O projeto é gratuito, sem assinatura, sem catálogo comercial embutido e sem telemetria.

A biblioteca é compartilhada entre 2–5 usuários, com playlists, favoritos e histórico segregados por pessoa. O Zuno não distribui música: os arquivos vêm do próprio usuário ou de fontes que ele tenha o direito de usar. A biblioteca usa **Opus 128 kbps VBR**: a conversão dos arquivos originais acontece no cliente que envia (Windows/Tauri, M4) e o servidor apenas armazena e transmite Opus ([ADR-0007](docs/adr/0007-opus-formato-canonico.md)). Integrações externas (metadados, IA, letras) são opcionais e nunca requisito para o funcionamento básico.

O marco **M0 (fundação)** entregou o monorepo, o healthcheck, as migrations SQLite, o pipeline de qualidade e esta documentação. O **M1a** adiciona o scanner da biblioteca (indexação de Opus, tags, capas, duplicatas e ausências). Streaming HTTP, upload, login e player continuam pendentes (M1b/M1c/M2).

> **Licença Apache-2.0.** O repositório é open-source sob [Apache-2.0](LICENSE), com atribuição em [NOTICE](NOTICE) — decisão registrada em [ADR-0006](docs/adr/0006-licenca-apache-2.0.md). Contribuições são bem-vindas sob esses termos.

## Status atual

| Item | Estado |
|---|---|
| M0 — Fundação | **Concluído** |
| M1 — Servidor musical (scan, tags, streaming) | **Em andamento** (M1a e M1b concluídos; M1c pendente) |
| M2 em diante | Não iniciado |

O que existe hoje:

- `zuno-server`: binário Rust (Axum + Tokio + SQLx/SQLite) com `GET /health`, scanner (`zuno-server scan`), API de biblioteca/busca em `/api/v1`, streaming com `Range` e capas redimensionadas.
- `@zuno/web`: página de conectividade React que consulta `/health`, com proxy de desenvolvimento para o backend.
- Design tokens e tipos/contrato de API compartilhados (`packages/`).
- Scaffold Tauri 2 em `apps/desktop/src-tauri` (não compilado no M0).
- Documentação de arquitetura, ADRs, threat model e OpenAPI mínimo.

## Stack

| Camada | Tecnologia |
|---|---|
| Backend | Rust stable (edição 2024), Axum 0.8, Tokio, SQLx 0.8 + SQLite (WAL) |
| Banco | SQLite com migrations embutidas (`server/migrations`) |
| Frontend | React 19, TypeScript strict, Vite 8, Tailwind CSS 4 |
| Testes/qualidade | `cargo fmt`/`clippy`/`test`, ESLint, Prettier, Vitest, GitHub Actions |
| Desktop/Android (planejado) | Tauri 2 |

## Pré-requisitos

- **Rust 1.88+** (stable, toolchain MSVC no Windows) — `rustup` recomendado.
- **Node.js 22.12+** e **npm 10+**.
- **Windows 10/11** ou **Linux** (incluindo ARM para Raspberry Pi).
- No Windows: MSVC Build Tools (linker `link.exe`) e WebView2 (necessário apenas para o Tauri, futuro).
- No Linux: `build-essential` e `pkg-config`. O SQLite é compilado/embutido via `libsqlite3-sys`; não é necessário instalar o servidor SQLite.

Passo a passo detalhado: [docs/installation/development.md](docs/installation/development.md).

## Quickstart (dois terminais)

O backend e o frontend rodam em processos separados. Abra **dois terminais** na raiz do repositório.

### PowerShell (Windows)

Terminal 1 — backend:

```powershell
cargo run -p zuno-server
```

Terminal 2 — frontend:

```powershell
npm ci
npm run dev
```

### bash (Linux/macOS)

Terminal 1 — backend:

```bash
cargo run -p zuno-server
```

Terminal 2 — frontend:

```bash
npm ci
npm run dev
```

Depois acesse <http://127.0.0.1:5173>. O Vite faz proxy de `/health` para `http://127.0.0.1:8080`. Na primeira execução o backend cria `data/zuno.db` e aplica as migrations.

Verificação direta do backend:

```bash
curl http://127.0.0.1:8080/health
```

```json
{
  "status": "ok",
  "service": "zuno-server",
  "version": "0.1.0",
  "uptime_seconds": 12,
  "database": { "status": "ok" }
}
```

Se o banco não responder, a resposta é **503** com `"status": "degraded"` e `"database": { "status": "error" }`. Erros da API usam o formato `{ "code": "snake_case", "message": "texto" }`.

## Comandos

| Comando | O que faz |
|---|---|
| `cargo run -p zuno-server` | Sobe o backend em `127.0.0.1:8080` |
| `cargo run -p zuno-server -- scan` | Varre `ZUNO_MUSIC_DIR` e imprime o relatório em JSON |
| `cargo test --workspace` | Testes Rust |
| `cargo fmt --all --check` | Verifica formatação Rust |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lint Rust (warnings são erros) |
| `npm ci` | Instala dependências dos workspaces npm |
| `npm run dev` | Servidor de desenvolvimento web em `127.0.0.1:5173` |
| `npm run typecheck` | TypeScript strict nos workspaces |
| `npm run lint` | ESLint |
| `npm run test` | Vitest |
| `npm run build` | Build de produção do frontend |
| `npm run format:check` | Verifica Prettier |
| `npm run check` | Pipeline completa do frontend (format, typecheck, lint, test, build) |

## Configuração

O backend lê variáveis de ambiente (veja `.env.example`):

| Variável | Padrão | Descrição |
|---|---|---|
| `ZUNO_BIND_ADDR` | `127.0.0.1:8080` | Endereço de bind. O padrão é local e **não exposto publicamente** |
| `ZUNO_DATABASE_URL` | `sqlite://data/zuno.db` | Banco SQLite (criado automaticamente) |
| `ZUNO_MUSIC_DIR` | `./data/music` | Biblioteca **somente Opus**; o scan nunca move nem apaga arquivos |
| `ZUNO_ARTWORK_DIR` | `./data/artwork` | Cache de capas extraídas (nomeadas por sha256) |
| `ZUNO_MAX_AUDIO_FILE_BYTES` | `524288000` | Limite por arquivo no scan (500 MiB) |
| `ZUNO_SCAN_ON_STARTUP` | `false` | Varre a biblioteca antes de servir |
| `ZUNO_DB_MAX_CONNECTIONS` | `4` | Conexões máximas no pool (mantenha baixo no Pi 3) |
| `RUST_LOG` | — | Filtro de logs (`tracing`) |
| `VITE_ZUNO_API_BASE_URL` | vazio | URL base da API no frontend; vazio = mesma origem/proxy |
| `ZUNO_DEV_API_TARGET` | `http://127.0.0.1:8080` | Destino do proxy `/health` do Vite (use se a porta 8080 estiver ocupada) |

Não comite `.env` nem segredos. O servidor escuta apenas em loopback por padrão; expor na rede exige decisão consciente, TLS e autenticação (a partir do M2).

## Estrutura do repositório

```text
zuno-streaming/
├── server/                 # backend Rust (binário zuno-server)
│   ├── migrations/         # migrations SQLx embutidas
│   ├── src/                # main, config, api, db, error
│   └── tests/              # testes de integração
├── apps/
│   ├── web/                # frontend React + Vite (workspace @zuno/web)
│   └── desktop/src-tauri/  # scaffold Tauri 2 (não compilado no M0)
├── packages/
│   ├── api-client/         # cliente HTTP tipado
│   ├── design-tokens/      # tokens de design (CSS/TS)
│   └── shared-types/       # tipos compartilhados do contrato
├── docs/
│   ├── adr/                # decisões arquiteturais
│   ├── api/                # contrato OpenAPI
│   ├── architecture/       # visão geral e matriz de áudio
│   ├── installation/       # setup de desenvolvimento
│   ├── milestones/         # registro de marcos
│   ├── rfcs/               # propostas (RFC) e template
│   └── security/           # threat model
├── scripts/                # utilitários de desenvolvimento
├── .github/workflows/      # CI (Linux + Windows) e auditoria de dependências
├── Cargo.toml              # workspace Rust
├── package.json            # workspaces npm
├── PRD.md                  # requisitos do produto (fonte de verdade)
└── PLANO_AGENTE.md         # plano de implementação e milestones
```

A árvore acima reflete apenas diretórios e arquivos existentes no marco M0; arquivos de marcos futuros serão adicionados quando implementados.

## Testes e CI

O GitHub Actions executa, em Linux e Windows:

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`;
- `npm ci`, `npm run format:check`, `npm run typecheck`, `npm run lint`, `npm run test`, `npm run build`;
- auditoria de dependências (`cargo audit` + `npm audit`) em workflow separado.

Nenhuma métrica de desempenho ou cobertura é publicada enquanto não houver medição real. Benchmarks no Raspberry Pi 3 serão registrados quando o hardware de referência for confirmado.

## Roadmap

| Marco | Escopo | Estado |
|---|---|---|
| [M0 — Fundação](PLANO_AGENTE.md#m0--fundação-antes-de-funcionalidades) | Monorepo, CI, healthcheck, migrations, OpenAPI, ADRs, threat model | Concluído |
| M1 — Servidor musical | Scan de biblioteca, tags, import jobs, streaming HTTP Range | Não iniciado |
| M2 — Web core | Auth/convites, biblioteca, busca, playlists, upload, player | Não iniciado |
| M3 — Web offline/áudio | PWA offline, gapless/crossfade/EQ por capacidade, backup | Não iniciado |
| M4 — Windows MVP | Tauri 2, instalador/portátil, integrações desktop | Não iniciado |
| M5 — Android | Serviço de áudio, notificações, offline, APK/F-Droid | Não iniciado |
| M6 — Zuno Connect | Sessões multi-device, comandos remotos, sync | Não iniciado |
| M7 — IA e descoberta | Recomendações locais, BYOK, rádio, smart downloads | Não iniciado |
| M8 — Ecossistema | Letras, catálogo externo autorizado, i18n ampliado | Não iniciado |

Detalhes, gates e backlog: [`PLANO_AGENTE.md`](PLANO_AGENTE.md). Requisitos: [`PRD.md`](PRD.md).

## Documentação

- [Instalação e desenvolvimento](docs/installation/development.md)
- [Visão de arquitetura](docs/architecture/overview.md)
- [Matriz de capacidades de áudio](docs/architecture/audio-capabilities.md)
- [Contrato OpenAPI](docs/api/openapi.yaml)
- [Threat model](docs/security/threat-model.md)
- [ADRs](docs/adr/README.md)
- [RFCs (propostas)](docs/rfcs/README.md)
- [Marco M0](docs/milestones/M0.md)
- [Como contribuir](CONTRIBUTING.md) · [Segurança](SECURITY.md)

## Licença

[Apache-2.0](LICENSE) — Copyright 2026 Zuno contributors (ver [NOTICE](NOTICE)).
A decisão está registrada em [ADR-0006](docs/adr/0006-licenca-apache-2.0.md).

Você pode usar, modificar e redistribuir o Zuno, inclusive em produtos proprietários, desde que preserve
os avisos de licença e o `NOTICE`. Não há garantia: o software é fornecido “AS IS”.

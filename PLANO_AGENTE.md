# Zuno — Plano de Implementação para Agentes de Programação

**Versão:** 1.0 · **Data:** 25/10/2026  
**Fonte normativa:** `PRD.md` v2.0  
**Papel:** manual executável de engenharia, backlog de issues, arquitetura de referência e instruções para `AGENTS.md`.

> **IMPORTANTE:** Este arquivo é o segundo de **dois documentos entregues**. Contém o conteúdo e as regras que podem ser copiados para `AGENTS.md` posteriormente. Não substitua requisito confirmado por preferência do agente. Não implemente uma integração de origem ou direitos incertos só porque ela aparece no roadmap.

## 0. Como um agente deve iniciar o trabalho

1. Ler **inteiramente** `PRD.md` e este plano antes de criar código.
2. Inspecionar estado real do repositório. Se vazio, registrar que se trata de bootstrap; **não pressupor** arquivos, packages ou infraestrutura.
3. Executar comandos de inspeção não destrutivos para descobrir SO, toolchains e ambiente. No Windows, preferir PowerShell; no Pi, bash.
4. Conferir seção "Pendências" do PRD. Se uma feature depende de decisão pendente, implementar apenas infraestrutura neutra ou registrar issue; não escolher arbitrariamente.
5. Produzir uma proposta de mudanças focada em **um milestone ou issue**; não tentar entregar o produto inteiro em uma alteração.
6. Fazer testes e atualizar docs/API/migrations a cada modificação.
7. Relatar arquivos modificados, comandos rodados, resultados e limitações; jamais alegar teste real em Pi/Android quando só simulou.

### Convenções de autoridade

1. Segurança, lei e direitos de terceiros.
2. Requisitos **[CONFIRMADO]** de `PRD.md`.
3. ADRs aprovadas no repositório.
4. Contratos OpenAPI/migrations versionados e testados.
5. Este plano e issues aceitas.
6. Convenções do código existente.

Em conflitos, preservar dados e compatibilidade, registrar a discrepância e propor ADR. **Não adicionar APIs de download de fontes sem autorização, scrapers que contornem limitações, coleta oculta ou IA paga por padrão.**

## 1. Definição de arquitetura executável

### 1.1 Stack fechada

- Rust stable; `axum`, `tokio`, `sqlx` SQLite, `tracing` e `serde`; `lofty` para tags; bibliotecas auxiliares auditadas para codificação e inspeção de mídia.
- React, TypeScript strict, Vite e Tailwind CSS; UI reutilizável em Web/Tauri 2.
- Tauri 2 desktop e Android; adaptadores de áudio por plataforma quando necessário.
- SQLite WAL/FTS5 e filesystem para áudios/capas; engine de recomendação local básica no servidor e provedores cloud opcionais.
- REST `/api/v1`, HTTP Range streaming, WebSockets para controle/presença, OpenAPI como contrato.
- `systemd` para Linux, executável Windows, Docker Compose opcional; instalação web segura.
- GitHub Actions, Vitest, Playwright, Rust tests, Clippy, rustfmt, ESLint, SAST e auditorias de dependências/licenças.

### 1.2 Proposta de diretórios

```text
zuno/
├── PRD.md
├── PLANO_AGENTE.md
├── AGENTS.md                   # gerar/adotar conteúdo da seção 9
├── README.md
├── CONTRIBUTING.md
├── SECURITY.md
├── LICENSE                     # SOMENTE após escolha expressa
├── Cargo.toml                  # workspace Rust
├── package.json                # workspace frontend
├── pnpm-workspace.yaml         # se optar por pnpm; decisão operacional
├── server/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── config.rs
│   │   ├── api/
│   │   ├── auth/
│   │   ├── domain/
│   │   ├── repository/
│   │   ├── library/
│   │   ├── stream/
│   │   ├── sync/
│   │   ├── recommend/
│   │   ├── providers/
│   │   ├── backup/
│   │   └── jobs/
│   ├── migrations/
│   └── tests/
├── apps/
│   ├── web/
│   ├── desktop/src-tauri/
│   └── mobile/src-tauri/
├── packages/
│   ├── ui/
│   ├── player/
│   ├── api-client/
│   ├── design-tokens/
│   └── shared-types/
├── workers/                    # opcional, por demanda
├── docs/
│   ├── adr/
│   ├── api/
│   ├── security/
│   ├── architecture/
│   ├── installation/
│   └── performance/
├── scripts/
└── .github/workflows/
```

**[PROPOSTO]** `pnpm` pode facilitar workspaces, mas o proprietário usa Node/npm; agente deve checar preferência e manter npm se não houver ganho justificado. Não instalar serviços pesados para resolver tarefas simples.

### 1.3 Boundaries e contratos

- Camada domínio isolada de Axum, HTTP e SQLx.
- Repositórios encapsulam SQLite. Todas as mudanças usam migrations; não construir schema ad hoc em runtime.
- `AudioStorage` abstrai arquivos locais; `MetadataProvider`, `LyricsProvider`, `RecommendationProvider` e `TranscodeWorker` são interfaces, não dependências obrigatórias.
- `PlaybackAdapter` no cliente implementa capacidades por plataforma (seek, gapless, EQ, crossfade, volume, background, download/offline).
- `SyncEngine` consome eventos versionados; fila de comandos remotos usa IDs e versões para evitar replay/race.
- Todo endpoint recebe um contexto explícito de autorização e ownership. Moderação não equivale a superusuário.

## 2. Roadmap operacional e ordem de implementação

### M0 — Fundação (antes de funcionalidades)

**Objetivo:** inicializar repositório, padrões, CI e base de documentação.  
**Dependências:** licença **decidida em 25/10/2026: Apache-2.0** (`LICENSE` + `NOTICE` publicados); contribuições externas liberadas sob os termos da licença.

> **Status (2026-10-25): concluído e publicado em https://github.com/xxyurirangelxx/zuno-streaming com CI e Security verdes (Linux + Windows + frontend). Aguardando o “ok” do proprietário para iniciar o M1.**
> Evidências completas em [`docs/milestones/M0.md`](docs/milestones/M0.md).

- [x] Criar workspaces Rust/TS, README, docs de setup, CONTRIBUTING e SECURITY.
- [x] Criar skeleton `server` e `apps/web` com healthcheck.
- [ ] Definir esquema mínimo OpenAPI e gerador do cliente tipado. — **parcial:** `docs/api/openapi.yaml` pronto; cliente tipado (`packages/api-client`) escrito à mão e testado; geração automática (ZUNO-003) permanece pendente.
- [x] Adicionar pipeline CI: fmt, clippy, Rust tests, TS typecheck, ESLint, Vitest, security scan. *(executado no GitHub: Backend Linux, Backend Windows, Frontend e Security verdes)*
- [x] Instalar framework de migrations SQLite, `PRAGMA foreign_keys=ON`, WAL e índices iniciais. *(índices de domínio só fazem sentido com o schema do M1)*
- [x] Registrar ADR-0001 (monólito Rust), ADR-0002 (SQLite), ADR-0003 (client architecture), ADR-0004 (storage). *(também ADR-0005 ferramentas, ADR-0006 Apache-2.0, ADR-0007 Opus canônico, ADR-0008 M1 sem auth)*
- [x] Criar tokens iniciais de design e mecanismo reduced-motion/reduced-transparency.

**Gate M0:** build em Windows e Linux; healthcheck e migrations com teste; docs permitem subir dev environment; nenhuma chave hardcoded.

**Resultado do gate:** Windows verificado localmente (build, clippy, 14 testes Rust, 18 testes de frontend, build de produção e integração backend↔proxy); Linux/Windows no GitHub Actions verdes; `cargo audit`/`npm audit` verdes; `main` protegido com os 5 checks obrigatórios.

### M1 — Servidor musical

> **Decisões fechadas (25/10/2026):** biblioteca **somente Opus 128 kbps VBR** (ADR-0007); servidor **não converte** — a conversão é do cliente Windows/Tauri (M4); M1 **sem autenticação**, bind loopback (ADR-0008); entrega em 3 PRs (M1a schema+scanner, M1b streaming, M1c upload/staging); fixtures de áudio sintéticas geradas com ffmpeg; alvo armv7 (Pi 3 B 32-bit) com hardware pendente.
>
> **Status (26/10/2026): M1a, M1b e M1c concluídos** — evidências em [`docs/milestones/M1a.md`](docs/milestones/M1a.md), [`M1b`](docs/milestones/M1b.md) e [`M1c`](docs/milestones/M1c.md). Falta apenas a medição em Pi 3 B (hardware pendente) para fechar o gate.

- [x] Configuração validada com caminhos, limites e permissões de SO. *(M1a: `ZUNO_MUSIC_DIR`, `ZUNO_ARTWORK_DIR`, `ZUNO_MAX_AUDIO_FILE_BYTES`, `ZUNO_SCAN_ON_STARTUP`)*
- [x] Entity model: Track/Artist/Album/AudioFile/Artwork e index FTS. *(M1a: migration `0002` + FTS5)*
- [x] Scanner incremental + manual + reentrada após interrupção; **aceitar apenas Opus** e reportar os demais formatos sem mover/apagar. *(M1a: `zuno-server scan`, idempotente por mtime+tamanho)*
- [x] Extração de tags com Lofty; preservação de provenance; filenames fallback. *(M1a: título pelo nome do arquivo quando não há tag)*
- [x] Proteger import por staging, tamanho, MIME/probe, checksum, symlink/path traversal; **rejeitar upload não-Opus (415)**. *(M1c: staging fora da biblioteca, sha256, 413/415/422, duplicata 409)*
- [x] Tratar duplicatas e ausência de arquivo sem deleção automática. *(M1a: `duplicate_exact` + `available=0`)*
- [x] Streaming GET/HEAD com Range e cancelamento (sem auth no M1; loopback only). *(M1b: 206/416/If-Range/HEAD)*
- [x] Capa embutida e reduzida em cache; fallback. *(M1a extração; M1b resize 320/640 + cache imutável)*
- [x] Integração API list/search/filter/cursor e documentação. *(M1b: `/api/v1/tracks|albums|artists|search` + OpenAPI 0.2.0)*

**Gate M1:** servidor serve **Opus** em LAN com seek e restrições; formatos diferentes de Opus são rejeitados/ignorados com relatório claro e **sem deleção**; testes de seek, arquivos ausentes e uploads interrompidos passam. Medição em hardware: Pi 3 B (armv7) **pendente** até o hardware estar disponível — até lá, cross-compile + CI + x86-64.

### M2 — Web funcional, auth e biblioteca

> **Status (26/10/2026): M2a concluído** (auth/RBAC/convites) — evidências em [`docs/milestones/M2a.md`](docs/milestones/M2a.md). Frontend (M2b–M2e) pendente.

- [x] Instalação de primeiro admin local com código único e expiração. *(M2a)*
- [x] Login senha/Argon2id, sessão refresh revogável e convite com validade. *(M2a)*
- [x] Papéis: admin/moderador/usuário, defaults restritivos, testes IDOR. *(M2a: 110 testes, incluindo 401/403/CSRF)*
- [ ] Web shell, temas claro/escuro/sistema e navegação responsiva.
- [ ] Música, álbum, artista, capa, search instantânea e filtros.
- [ ] Player com fila, shuffle, repeat, seek e teclas básicas.
- [ ] Playlists por usuário e favoritos separados; compartilhamento somente autorizado.
- [ ] Web upload progressivo com revisão de duplicatas.
- [ ] Preferências, privacidade, histórico individual.

**Gate M2:** admin configura instância, convida usuário, importa pasta, pessoa toca música, cria playlist, sai/reentra; outro usuário não consegue consultar seus dados privados.

### M3 — Offline web e playback avançado

- [ ] Prova de conceito Web Audio/HTMLAudio e recursos por navegador.
- [ ] Gapless/crossfade/equalizador/ReplayGain onde tecnicamente compatíveis.
- [ ] Feature detection explícita; explicar degradações na UI.
- [ ] Cache e gerenciamento offline PWA com quota e integridade por checksum.
- [ ] Reprodução offline de downloads existentes, sem prometer edição de biblioteca sem servidor.
- [ ] Recomendação local/autoplay funcional sem cloud.
- [ ] Backup consistente exportável e restauração guiada inicial.
- [ ] Assistente para acesso remoto via VPN/túnel sem exigir abertura de porta.

**Gate M3:** testes em Chrome/Edge/Firefox ou browsers explicitamente suportados; offline verificável com servidor desligado; sem regressão em acessibilidade e segurança.

### M4 — Windows e MVP público

- [ ] App Tauri 2 Windows com UI compartilhada.
- [ ] Integrações: mídia/atalhos, bandeja, autostart opcional, controles de tarefa, Discord opcional.
- [ ] Instalador e build portátil; desinstalação não remove biblioteca no servidor.
- [ ] **Conversão client-side para Opus 128 kbps VBR** antes do envio (ffmpeg embutido), com original preservado no cliente e aviso para fontes lossy (ADR-0007).
- [ ] Downloads e offline Windows, gerenciamento de storage e exportação quando configurada.
- [ ] Áudio avançado validado em Windows; fallback no que não funcionar.
- [ ] Verificação de update assinada com fluxo seguro e changelog.
- [ ] Docs PT-BR/EN, screenshots, troubleshooting, artefatos com checksum.

**Gate MVP:** fluxo E2E real de instalação → pairing → import → playlists → reprodução remota → offline → restore; release notes descrevem o que é P0 e o que permanece limitado em PWA.

### M5 — Android

- [ ] Tauri 2 Android build com versão suportada por F-Droid; auditar plugins de terceiros.
- [ ] Adapter nativo de áudio para background/controle mídia/audio focus/Bluetooth.
- [ ] Download com Wi-Fi default, permissão dados móveis e quota.
- [ ] Reconciliação de fila/estado no retorno; notificações e widgets em sub-milestones.
- [ ] APK assinado no GitHub; pipeline F-Droid após validar licenças e reprodutibilidade.
- [ ] Início após boot apenas nas formas permitidas pelo Android, sem forçar reprodução.

**Gate M5:** teste em dispositivo Android real, tela bloqueada, ligação interrompendo áudio, fones, download interrompido e reconexão.

### M6 — Zuno Connect

- [ ] Dispositivo e sessão autenticados, heartbeat WebSocket.
- [ ] Sessões do usuário e principal selecionável, listas de dispositivos.
- [ ] Comandos idempotentes, revisions/acks, snapshot REST e event log incremental.
- [ ] Transferência direta sem encaminhar o stream via outro cliente.
- [ ] Resolução de conflitos de múltiplas telas e deslogados, capabilites por destino.

**Gate M6:** nenhum usuário controla sessão de outro; transferência mantém faixa/fila/posição dentro da precisão observada e reconcilia desconexões.

### M7 — Recomendações e automação inteligente

- [ ] Modelo de eventos histórico/skip/like com controles de retenção.
- [ ] Ranking local com diversificação e cold-start.
- [ ] Cache persistente de recomendações e invalidation.
- [ ] Provider API BYOK (Gemini, OpenAI, OpenRouter, Hugging Face, Ollama remoto, endpoints custom) por interface comum.
- [ ] Permission gate de dados de histórico por usuário, quotas/budget, secrets e logs redigidos.
- [ ] Features acústicas extraídas em PC/worker, sem bloquear Pi.
- [ ] Rádio faixa/artista, mixes e playlists por regras/IA, mood, smart downloads.

**Gate M7:** zero API cloud funciona; opt-out impede envio de histórico; provedor inválido ou sem créditos não interrompe autoplay.

### M8 — Ecossistema e polish

- [ ] Letras incorporadas e arquivos LRC, LRC por linha/palavra quando disponível.
- [ ] Provedores externos autorizados com prioridade e cache definidos.
- [ ] Adapter Monochrome somente se pesquisas confirmarem APIs, licença e termos de uso.
- [ ] Edição robusta de tags e organização gerenciada com rollback em operações destrutivas.
- [ ] Extensão i18n comunitária, refinamento Liquid Glass e acessibilidade.
- [ ] Otimizações conforme benchmarks reais, não suposições.

## 3. Backlog de issues (pronto para GitHub Projects)

Cada item possui ID estável, milestone, prioridade e teste de aceite. Dividir em issues menores após inspeção do repositório. **Nunca começar P1/P2 para contornar P0 quebrado.**

| Issue | Marco | Prio | Título | Critério mínimo de aceite |
|---|---|---|---|---|
| ZUNO-001 | M0 | P0 | Inicializar monorepo | builds Rust/TS e README local |
| ZUNO-002 | M0 | P0 | CI de lint/test/security | checks bloqueiam merge com falhas |
| ZUNO-003 | M0 | P0 | OpenAPI + client gerado | tipos e testes de contrato sincronizados |
| ZUNO-004 | M0 | P0 | SQLite migrations | migra do zero e reinicia idempotente |
| ZUNO-005 | M1 | P0 | Scanner de biblioteca | insere/atualiza sem duplicatas não resolvidas |
| ZUNO-006 | M1 | P0 | Extração de tags e capas | arquivos incompletos não derrubam worker |
| ZUNO-007 | M1 | P0 | Streaming HTTP Range | HEAD/206/416 e seek testados |
| ZUNO-008 | M1 | P0 | Upload e staging seguro | rejeita traversal, arquivos excessivos e interrupção |
| ZUNO-009 | M1 | P0 | Busca FTS e paginação | busca encontra artista/álbum/faixa com cursor |
| ZUNO-010 | M2 | P0 | Bootstrap seguro | código único expira e setup fecha depois do claim |
| ZUNO-011 | M2 | P0 | Login, convite e RBAC | cenários IDOR e moderação testados |
| ZUNO-012 | M2 | P0 | Layout e temas | dark/light/system e teclado acessível |
| ZUNO-013 | M2 | P0 | Player e fila web | skip, seek, repeat, shuffle e erros |
| ZUNO-014 | M2 | P0 | Playlists e favoritos | segregação usuário e compartilhamento correto |
| ZUNO-015 | M2 | P0 | Upload UI/admin | import conflict review em lote |
| ZUNO-016 | M3 | P0 | PWA offline | músicas baixadas tocam sem servidor suportado |
| ZUNO-017 | M3 | P0 | Playback avançado web | capability matrix + fallback visível |
| ZUNO-018 | M3 | P0 | Autoplay local | continua fila sem API externa |
| ZUNO-019 | M3 | P0 | Backup/restore | backup validado e restauração em ambiente isolado |
| ZUNO-020 | M3 | P0 | Setup de rede remota | informa CGNAT, túnel/VPN e HTTPS |
| ZUNO-021 | M4 | P0 | Windows Tauri | app distribuível e estável |
| ZUNO-022 | M4 | P0 | Windows portable/installer | ambos abrem, iniciam e desinstalam corretamente |
| ZUNO-023 | M4 | P0 | Desktop playback/offline | atalho mídia, cópia offline, efeitos conforme suporte |
| ZUNO-024 | M4 | P0 | Preparar release público | checksums, segurança e changelog |
| ZUNO-025 | M5 | P1 | Android media service | fundo/bloqueio/foco áudio comprovados |
| ZUNO-026 | M5 | P1 | Android downloads | Wi-Fi default e quota persistentes |
| ZUNO-027 | M5 | P1 | F-Droid publication path | auditado e pipeline compatível |
| ZUNO-028 | M6 | P1 | WebSocket session engine | reconecta e rejeita comandos estranhos |
| ZUNO-029 | M6 | P1 | Connect remote control | transferência e ownership demonstrados |
| ZUNO-030 | M7 | P1 | Recommendation local ranking | cold-start e autoplay estável |
| ZUNO-031 | M7 | P1 | BYOK provider adapter | chaves nunca expostas ao cliente |
| ZUNO-032 | M7 | P1 | Consent + cloud history opt-out | payload remoto sem histórico opt-out |
| ZUNO-033 | M7 | P1 | Smart mixes/downloads | quota e previsão não baixam além do limite |
| ZUNO-034 | M8 | P1 | Lyrics aggregator | LRC local e provedores licenciados |
| ZUNO-035 | M8 | P2 | Monochrome feasibility | relatório de API/termos; implementar só se aprovado |
| ZUNO-036 | M8 | P1 | Tag writeback seguro | original não se perde em falha |
| ZUNO-037 | Cross | P0 | Restore and migration tests | esquema antigo migra/recupera dentro da política |
| ZUNO-038 | Cross | P0 | Privacy/security regression | roles, consent e revogação sem regressão |
| ZUNO-039 | Cross | P0 | Hardware diagnostics | coleta métricas localmente, sem telemetria |
| ZUNO-040 | Cross | P1 | Accessibility/i18n audit | teclado/leitores/reduced motion PT-BR/EN |

### Template obrigatório de issue

```markdown
## Objetivo
## Escopo / fora de escopo
## Requisitos do PRD
## Interfaces e dados afetados
## Dependências / riscos
## Implementação proposta
## Critérios de aceite observáveis
- [ ] Comportamento positivo
- [ ] Erros e fallback
- [ ] Segurança e permissões
- [ ] Testes automatizados
- [ ] Documentação e logs
## Evidências de validação
## Rollback (se altera DB/arquivos)
```

## 4. ADRs

Um arquivo por ADR em `docs/adr/` com Contexto, Decisão, Alternativas, Consequências e Status. Índice em [`docs/adr/README.md`](docs/adr/README.md).

**Registradas:**

- **ADR-0001 [Aceita]:** Rust/Axum/Tokio como backend, monólito modular.
- **ADR-0002 [Aceita]:** SQLite principal, migrations SQLx embutidas, sem PostgreSQL no MVP.
- **ADR-0003 [Aceita]:** React/TS/Vite/Tailwind compartilhado; Web/PWA primeiro, Tauri 2 depois; capacidades por plataforma.
- **ADR-0004 [Aceita]:** áudio no filesystem e metadados no SQLite; original preservado; IDs independentes de caminho.
- **ADR-0005 [Aceita]:** ferramentas do monorepo (npm workspaces, TypeScript 5.9, ESLint 10, Prettier, Tailwind 4, Vitest).
- **ADR-0006 [Aceita]:** licença **Apache-2.0** (25/10/2026).
- **ADR-0007 [Aceita]:** **Opus 128 kbps VBR** como formato canônico; conversão no cliente Windows/Tauri (M4); servidor não transcodifica.
- **ADR-0008 [Aceita, temporária]:** M1 sem autenticação, bind loopback; autorização obrigatória a partir do M2.

**Ainda pendentes (numeração a atribuir):** API REST versionada + WebSocket/sync incremental (M2/M6), IA BYOK e quotas (M7), VPN/túnel para CGNAT (M3), RP ID/passkeys (M2), storage offline e capacidades por motor (M3), Monochrome/letras (M8).

## 5. Segurança e compliance por tarefa

### 5.1 Checklist de endpoint

- [ ] Autenticação necessária? (somente `/health` sem segredo e endpoints de setup estritamente limitados)
- [ ] Escopo de tenant/servidor e ownership de usuário validado?
- [ ] Papel permite ação? Moderador com mínimo privilégio?
- [ ] Quais dados são pessoais e entram em logs?
- [ ] Input validado com limites de tamanho, paths e tokens?
- [ ] Rate limit e idempotência em operações sensíveis?
- [ ] CORS/CSRF/SameSite corretos conforme canal?
- [ ] Testes de negação (401/403/404) não revelam entidade de terceiro?

### 5.2 Checklist de serviço externo

- [ ] A API é documentada e autorizada? Não depender de engenharia reversa contra limites ou bypass.
- [ ] Licença/ToS permite o uso? Conteúdo musical/letras pode ter copyright.
- [ ] Dado pessoal enviado tem permissão e necessidade?
- [ ] Histórico privado de opt-out é excluído do payload?
- [ ] Chave está no servidor e nunca no frontend?
- [ ] Quotas, custos, timeout, retry, cache e desabilitar por config?
- [ ] O servidor continua funcional se o serviço cair?

### 5.3 Checklist de dados locais

- [ ] Gravações atômicas onde apropriado e rollback possível.
- [ ] Deleção física requer confirmação autenticada, registrada.
- [ ] Backup e restore têm manifesto e integridade.
- [ ] Segredos não entram em backup sem proteção adequada.
- [ ] Quota de microSD inclui temporários, covers, SQLite e SO.

## 6. Testes e matriz de capacidades

| Componente | Tipos de teste | Aceite |
|---|---|---|
| Backend HTTP | unit/integration/contract | erros previsíveis, autorização correta |
| Streaming | test range/seek/cancel/interruption | respostas conformes e sem leaks |
| Library scanner | fixture mp3/flac/opus/malformed | reentrância e consistência |
| Database | migrations/transactions/failure | backup/restauração íntegros |
| Web UI | Vitest + Playwright | acessibilidade, fila, playlists, offline |
| Windows | packaging/install/uninstall/media keys | instalador e portátil |
| Android | device tests | áudio background/foco/notificação |
| AI | mock providers, quotas, consent | fallback sem custo ou perda de privacidade |
| Sync | race/replay/reconnect | revisions e idempotência |
| Security | auth/IDOR/CSRF/traversal/secrets | defaults fechados |

**Matriz por plataforma:** mantenha `docs/architecture/audio-capabilities.md` com colunas Windows Tauri/Web Edge/Web Chrome/Web Firefox/Android e linhas `play`, `seek`, `gapless`, `crossfade`, `eq`, `replaygain`, `background`, `media_controls`, `offline`, `export`, `auto_update`. Estados: `supported`, `limited`, `unsupported`, `not-tested`, com versão, data e evidência. Nunca marcar `supported` só por expectativa.

**Performance:** apesar de não haver metas rígidas, registrar CPU/RAM/disk I/O, tempo de inicialização, latência de busca, seek, tempo de import e throughput em Pi 3 B/B+ (quando identificar hardware), Raspberry OS 32/64 quando viável e Windows. Evitar benchmarks falsos.

## 7. Guia de instalação e operação (plano da documentação)

### 7.1 Instalação Linux/Raspberry Pi — fluxo pretendido

1. Detectar arquitetura, distro, modelo de Pi e conectividade. Registrar versão.
2. Checar espaço de microSD, RAM, permissões, mount e conflitos de porta.
3. Instalar release verificada por checksum/assinatura em diretório de sistema.
4. Criar usuário de serviço `zuno`, diretórios de música, DB, cache e configs com permissões mínimas.
5. Criar unit `systemd` com hardening condizente com filesystem, watchdog/restart e logs limitados.
6. Subir interface de instalação **LAN restrita**, gerar código de pareamento temporário único; não expor internet pública no bootstrap.
7. Criar conta admin, configurar diretórios, regra Opus 128 VBR, política de preservação, import/backup.
8. Opcionalmente configurar VPN/túnel, explicar CGNAT, domínio estável, HTTPS e WebAuthn.
9. Fazer primeiro scan e healthcheck; exibir dicas de backup externo.
10. Oferecer mecanismos seguros de upgrade/uninstall sem apagar música sem confirmação.

> Esses são **passos a implementar**, não comandos de instalador já existentes. Não publique `curl | sh` de endereço inexistente.

### 7.2 Windows servidor

- Build nativo Rust, filesystem com paths Windows, SQLite ACLs, serviço Windows ou inicialização gerenciada.
- Wizard instalação e segurança da pasta de dados; firewall estritamente configurado mediante consentimento.
- Windows 10/11 clientes: instalador, portátil e atualizador separado com verificações.

### 7.3 Docker NAS/VPS opcional

- Imagem pequena multi-arch, user não-root, volumes persistentes separados de música/DB/cache, healthcheck, compose example.
- Documentar reverse proxy/TLS, backups e permissão de escrita, sem exigir Docker em Pi.

### 7.4 Troubleshooting e recuperação

Documentar cartão cheio, armazenamento removido, DB locked, sistema sem certificados, domínio alterado invalidando passkeys, upload interrompido, biblioteca duplicada, `Range` bloqueado em proxy, Android background kill, F-Droid update diferente de GitHub APK, troca de chave BYOK e restore incompatível.

## 8. Contribuições e releases

- GitHub público desde M0; issues com template acima e etiquetas `type`, `priority`, `milestone`, `area`, `security`, `needs-decision`.
- Branch protected com CI; PRs menores, testes e docs obrigatórios; revisão do mantenedor para auth, banco, mídia, rede e provedores.
- Política de divulgação responsável em `SECURITY.md`; segredos e logs sanitizados em bug reports.
- `CONTRIBUTING.md` com setup dev Windows/PowerShell e Linux/bash, convenções de commit e testes locais.
- Licença ainda pendente: **não afirmar que o código é MIT/Apache/AGPL até aprovação**. Comparação do PRD sugere Apache-2.0 para adoção permissiva ou AGPL-3.0 para reciprocidade de servidor; decisão final do dono.
- GitHub Releases: binários Linux ARM/x86-64/Windows, builds Tauri Windows (instalador + portátil), APK Android assinado; checksums/assinaturas, changelog, atualização segura.
- F-Droid exige sua própria validação; conexões a provedores proprietários devem ser completamente opcionais, sem SDK obrigatório nem bloqueio de uso básico.

## 9. Conteúdo recomendado para `AGENTS.md`

O bloco a seguir pode ser **copiado integralmente** para `AGENTS.md` quando o repositório for criado. Evite instruções que impeçam o agente de executar testes reais ou de apontar riscos.

```markdown
# AGENTS.md — Zuno

## Missão
Implementar o Zuno: streaming musical open-source self-hosted com servidor Rust leve, Web/Windows/Android, biblioteca multiusuário e privacidade. `PRD.md` é a fonte normativa de requisitos; `PLANO_AGENTE.md` define milestones e processo.

## Stack
- Server: Rust stable + Axum + Tokio + SQLx SQLite.
- UI: React + TypeScript strict + Vite + Tailwind.
- Apps: Tauri 2 (desktop/mobile); Web/PWA.
- API: REST versionada, streaming Range e WebSocket onde necessário.
- Architecture: monólito modular com workers opcionais; abstrações para providers e playback.

## Before coding
1. Read PRD and implementation plan.
2. Inspect actual repository and current tests; do not invent existing components.
3. Work on one issue/milestone at a time with a concrete acceptance checklist.
4. Never silently resolve a pending product decision.

## Safety
- No telemetry, paid cloud API, or proprietary dependency required for the core.
- Secrets only in server-side protected storage/env; never in git, frontend bundles or logs.
- Enforce RBAC/ownership server-side; user's listening history is private even from ordinary admins' application views.
- Honor user cloud-history opt-out for every recommendation provider.
- Never overwrite or delete original music without explicit authorized confirmation.
- No scraping, circumvention or downloading of third-party music/lyrics without demonstrated permission.
- Local first setup cannot be claimed remotely by strangers.

## Workflow
- Prefer minimal, composable changes and focused PRs.
- Add migration + rollback/restore analysis for data model changes.
- Add OpenAPI, client types, tests, and docs when modifying APIs.
- Validate error paths, network disconnects and no-cloud fallback.
- Always check accessibility and supported audio capabilities per platform.
- Run cargo fmt/clippy/test and frontend lint/typecheck/tests; report skipped checks truthfully.
- Document ADRs for decisions with significant long-term impact.

## Priority order
M0 foundation -> M1 streaming server -> M2 Web core -> M3 Web offline/audio -> M4 Windows MVP -> M5 Android -> M6 Connect -> M7 AI -> M8 optional integrations.

## Definition of Done
Code builds; tests pass; permissions/privacy reviewed; files preserved; config/docs updated; clear success and failure behavior; no hidden new dependencies or silently enabled cloud services.
```

## 10. Sequência recomendada para primeira sessão do agente

**Primeiro objetivo pequeno e verificável:** completar **ZUNO-001 + esqueleto de ZUNO-003**, sem importar música nem mexer em infraestrutura remota.

1. `git status` e inspeção do projeto.
2. Criar `server` com `/health` e configurações tipadas.
3. Criar `apps/web` com página de conectividade simples.
4. Compartilhar `baseUrl` configurável, sem tokens fake.
5. Pipeline mínimo format/lint/test/build.
6. README com comandos reais; rodar e registrar resultados.
7. Abrir ADRs de stack e lista de decisões pendentes.

**Prompt para iniciar o agente:**

> Leia `PRD.md` e `PLANO_AGENTE.md`. Inspecione este repositório. Execute somente a issue ZUNO-001 e o mínimo necessário para uma página Web que verifique `/health` de um servidor Rust/Axum. Use Rust + Axum + Tokio + SQLite migration scaffold e React + TypeScript + Vite + Tailwind. Não implemente ainda login, música, IA, Docker nem deploy público. Crie testes básicos e CI, documente comandos exatos para Windows PowerShell e Linux. Ao final reporte mudanças, testes realmente executados, riscos e próximos passos; não tome decisões pendentes como licença ou serviço de túnel.

## 11. Checklist de entrega do agente ao final de cada issue

- [ ] Requisito e milestone identificados
- [ ] Critérios de aceite associados a testes reproduzíveis
- [ ] RBAC/privacidade e mensagens de erro avaliadas
- [ ] Downgrade/fallback e operações offline avaliados se aplicável
- [ ] Docs/API/migrations e ADRs atualizados
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` executados (ou motivo da impossibilidade)
- [ ] TypeScript/ESLint/Vitest/E2E executados conforme camada (ou motivo)
- [ ] Nenhum segredo, dump de dados pessoais ou material protegido incluído
- [ ] Regressões, incompatibilidades e pendências explicitadas
- [ ] Resumo para o mantenedor, incluindo caminhos de arquivo e próxima issue recomendada

## 12. Itens a resolver com o proprietário antes de operações irreversíveis

- ~~Licença e arquivo LICENSE.~~ **Resolvido (25/10/2026): Apache-2.0.**
- Domínio permanente/RP ID para passkeys e a forma de acesso CGNAT padrão.
- ~~Modelo exato da placa Pi 3 e imagem Raspberry Pi OS testada.~~ **Resolvido: Pi 3 B 32-bit (armv7); hardware para medição pendente de disponibilidade.**
- Política e interfaces de exclusão de originais e backup externo. *(originais ficam no cliente; exclusão no servidor fora do escopo)*
- Direitos/APIs dos provedores de letras e Monochrome.
- Provedores de IA padrão, custos, retenção e privacidade.
- Política de atualizações automáticas do servidor com migrations incompatíveis.

O objetivo de um agente é **reduzir incerteza com provas de conceito e entregar pequenos incrementos testados**, e não maximizar a quantidade de código por alteração.

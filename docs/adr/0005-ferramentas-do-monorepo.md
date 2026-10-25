# ADR-0005 — Ferramentas e workspaces do monorepo

- **Status:** Accepted
- **Data:** 2026-10-25
- **Decisores:** proprietário do projeto
- **Fonte:** PLANO_AGENTE §1.2 e M0

## Contexto

O projeto combina backend Rust e frontend TypeScript, com pacotes compartilhados e um scaffold Tauri. É preciso escolher gerenciador de pacotes, versões de ferramentas e como os workspaces se organizam, mantendo o ambiente reproduzível em Windows (ambiente primário) e Linux (alvo de implantação), sem dependências desnecessárias.

## Decisão

- **npm workspaces** (`apps/web`, `packages/*`). **Sem pnpm/Yarn** — o proprietário usa Node/npm e não há ganho que justifique outra ferramenta.
- **TypeScript 5.9 (série 5.x)** com `strict`; **ESLint 10** (flat config) + **Prettier**; **Tailwind CSS 4**; **Vitest** para testes de frontend; **Vite 8**.
- **Cargo workspace** na raiz com o membro `server` (binário `zuno-server`), `edition = "2024"`, `rust-version = "1.88"`, `publish = false`.
- **Tauri 2 excluído do workspace Rust** (`exclude = ["apps/desktop/src-tauri"]`): depende de WebView2/GTK e não deve ser exigido em todo `cargo test`.
- Lints do workspace: `unsafe_code = "forbid"`, `missing_debug_implementations = "warn"`, clippy `all = warn` (CI eleva a `-D warnings`).
- Formatação com `rustfmt` e `.editorconfig`/Prettier para o restante.
- CI em GitHub Actions (Linux + Windows) + workflow de segurança (`cargo audit`, `npm audit --audit-level=high`).

## Alternativas consideradas

| Alternativa | Prós | Contras | Motivo da recusa |
|---|---|---|---|
| pnpm | instalação rápida, workspaces rigorosos | ferramenta extra, lockfile distinto | sem ganho justificado |
| Yarn Berry | PnP, performance | atrito com ferramentas e Tauri | complexidade desnecessária |
| Nx/Turborepo | cache de build | overhead para poucos pacotes | prematuro |
| Tauri no workspace Rust | um `cargo` só | exige libs gráficas em todo build/test | quebra CI headless |
| TypeScript 6/next | recursos novos | instabilidade | 5.x é estável e suportado |

## Consequências

- `npm ci` é o caminho reproduzível de instalação.
- O build do Tauri é separado e só entra em CI quando o desktop for implementado (M4).
- `Cargo.lock` e `package-lock.json` versionados garantem builds repetíveis.
- Atualizações de majors (ESLint, Tailwind, TS) exigem PR dedicado e revisão de breaking changes.
- Novas dependências precisam passar por auditoria e justificativa.

## Próximos passos

- Manter versões alinhadas entre `package.json` e documentação.
- Reavaliar a exclusão do Tauri se o build desktop passar a ser testado no CI.

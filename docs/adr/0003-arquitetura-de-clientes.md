# ADR-0003 — Arquitetura de clientes e capacidades por plataforma

- **Status:** Accepted
- **Data:** 2026-10-25
- **Decisores:** proprietário do projeto (via PRD v2.0 e `PLANO_AGENTE.md`)
- **Fonte:** PRD §2.3, §5.1 e §12; PLANO_AGENTE §6

## Contexto

O produto precisa de Web/PWA, Windows e Android com experiência consistente, mas os motores de áudio e as APIs de sistema diferem muito. Prometer paridade (gapless, crossfade, EQ, ReplayGain, background, controles de mídia) em todos os alvos seria irreal. A ordem confirmada é **Web/PWA primeiro, Windows depois, Android em seguida**.

## Decisão

- Uma única base de UI em **React + TypeScript strict + Vite + Tailwind**, compartilhada entre Web e Tauri.
- **Web/PWA primeiro**; Tauri 2 para desktop (Windows) e Android quando os milestones chegarem.
- **`PlaybackAdapter`** como interface de reprodução por plataforma: cada alvo implementa o que o motor permite, com fallback explícito.
- **Feature detection em runtime** + **matriz de capacidades** versionada em [`docs/architecture/audio-capabilities.md`](../architecture/audio-capabilities.md).
- A UI comunica degradações ao usuário; não esconder recurso indisponível.
- **Não prometer paridade de recursos** entre plataformas. Estados da matriz: `supported`, `limited`, `unsupported`, `not-tested` — e `supported` só com evidência real (versão, data, teste).

No M0 o frontend entrega apenas a página de conectividade (`/health`); o adaptador de reprodução é desenho para M3+.

## Alternativas consideradas

| Alternativa | Prós | Contras | Motivo da recusa |
|---|---|---|---|
| Apps nativos por plataforma | máximo acesso a APIs | três bases de UI, custo alto | inviável para o time |
| Só Web (sem Tauri) | simples | sem integração desktop/mobile, offline limitado | não atende M4/M5 |
| Framework cross-platform alternativo | um código | ecossistema menor, requisito Tauri 2 | fora do requisito |
| Prometer paridade total | marketing simples | frustra usuários e gera bugs | desonesto tecnicamente |

## Consequências

- Reuso alto de UI e tipos; esforço concentrado nos adaptadores nativos.
- Testes precisam cobrir cada motor suportado; a matriz começa toda em `not-tested`.
- Funcionalidades podem existir na Web e faltar no desktop/mobile (ou vice-versa), com aviso claro.
- O pacote Tauri fica fora do workspace Rust para não exigir WebView2/GTK em todo `cargo test`.

## Próximos passos

- Preencher a matriz com protótipos reais em M3 (Web) e M4/M5.
- Definir a interface `PlaybackAdapter` e os contratos de capacidade antes de implementar o player.

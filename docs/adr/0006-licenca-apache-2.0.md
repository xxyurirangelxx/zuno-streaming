# ADR-0006 — Licença Apache-2.0

- **Status:** Accepted
- **Data:** 2026-10-25
- **Decisores:** proprietário do projeto
- **Fonte:** PRD §2.1 e §17; PLANO_AGENTE §4 e §8

## Contexto

O Zuno é um projeto open-source self-hosted. Até 25/10/2026 a licença de reutilização estava **pendente**, o que
mantinha, por padrão, todos os direitos reservados: ninguém poderia copiar, modificar ou redistribuir legalmente,
mesmo com o repositório público. Isso bloqueava contribuições externas, releases e a própria promessa de projeto
open-source.

O PRD §17 registrou a comparação entre MIT, Apache-2.0, GPL-3.0 e AGPL-3.0 como base para a decisão.
As dependências atuais do Zuno (axum, tokio, sqlx, lofty, React, Vite, Tailwind, Tauri e demais) são MIT/Apache-2.0,
portanto compatíveis com qualquer das opções avaliadas.

## Decisão

**Licença Apache-2.0**, decidida pelo proprietário em 25/10/2026.

- `LICENSE` contém o texto oficial da Apache License 2.0 (sem modificações).
- `NOTICE` registra a atribuição: “Zuno — Copyright 2026 Zuno contributors”.
- `package.json` (raiz e workspaces) usa `"license": "Apache-2.0"`; os crates Rust declaram `license = "Apache-2.0"`.
- Contribuições externas passam a ser aceitas sob os termos da licença (ver `CONTRIBUTING.md`).
- Releases e binários podem declarar Apache-2.0.

## Alternativas consideradas

| Alternativa | Prós | Contras | Motivo da recusa |
|---|---|---|---|
| MIT | simples, adoção máxima | sem concessão explícita de patentes; sem reciprocidade | Apache-2.0 cobre patentes explicitamente com custo baixo |
| Apache-2.0 | permissiva, concessão de patentes, boa para ecossistema e plugins | permite fork fechado; exige avisos/NOTICE | **escolhida** |
| GPL-3.0 | garante abertura de derivados distribuídos | não cobre uso como serviço; atrito para integradores | copyleft mais rígido que o necessário para a visão do produto |
| AGPL-3.0 | fecha a brecha de SaaS | afasta integradores e alguns colaboradores | prioridade é adoção/reuso, não reciprocidade de servidor |
| Manter sem licença | nenhuma decisão precipitada | “todos os direitos reservados”; impede reuso e contribuições | contrário ao objetivo open-source do PRD |

## Consequências

- O projeto passa a ser **open-source de fato**, com permissão de uso, modificação e redistribuição.
- Forks proprietários são permitidos; em troca, há concessão expressa de patentes e obrigações de aviso.
- A auditoria de dependências deve continuar garantindo compatibilidade (Apache-2.0 é compatível com MIT/BSD/Apache;
  dependências GPL/AGPL no núcleo exigiriam revisão).
- `NOTICE` deve ser preservado em redistribuições; arquivos de terceiros com avisos próprios precisam ser mantidos.
- Nada muda em privacidade, telemetria ou segurança: a licença não afeta o comportamento do software.

## Status e próximos passos

- **Vigente.** Revisar apenas se surgir necessidade real (ex.: adoção de dependência incompatível ou mudança de
  estratégia do projeto), caso em que uma nova ADR substitui esta.
- Este documento não constitui aconselhamento jurídico.

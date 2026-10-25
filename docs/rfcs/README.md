# RFCs

Este diretório registra **propostas de trabalho** (RFCs) antes da implementação, conforme o marco M0 do PRD
(“monorepo, CI, RFCs, OpenAPI, design tokens, threat model”).

## RFC ou ADR?

| Documento | Pergunta que responde | Quando usar |
|---|---|---|
| **RFC** (`docs/rfcs/`) | “Deveríamos fazer isto? Como, com que alternativas e riscos?” | Antes de iniciar um trabalho com decisões em aberto, impacto em segurança/dados ou mais de uma abordagem viável |
| **ADR** (`docs/adr/`) | “O que foi decidido e por quê?” | Quando uma decisão arquitetural é tomada e passa a valer para o código |

Uma RFC aprovada normalmente gera uma ou mais ADRs. Decisões já confirmadas no `PRD.md` não precisam de RFC:
vão direto para ADR (quando arquiteturais) ou para a issue do milestone.

## Fluxo

1. Copie `0000-template.md` para `NNNN-titulo-curto.md` (numeração sequencial).
2. Abra uma issue/PR com a proposta e o link para o arquivo.
3. Status inicial: `Proposed`.
4. Após revisão do proprietário/mantenedor: `Accepted`, `Rejected` ou `Superseded by RFC-NNNN`.
5. Ao implementar, registre a decisão resultante em `docs/adr/` e marque a RFC como `Implemented`.

## Regras

- Uma RFC nunca autoriza violar o PRD, a segurança ou direitos de terceiros.
- Decisões pendentes listadas no PRD (§13) — licença, RP ID, hardware de referência, conectividade CGNAT,
  provedores externos — só avançam com aprovação expressa do proprietário.
- Não inclua segredos, dados pessoais, dumps ou arquivos de música nas propostas.

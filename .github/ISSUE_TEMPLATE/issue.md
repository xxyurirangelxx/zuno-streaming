---
name: Tarefa do plano
about: Proposta de trabalho alinhada ao PRD e ao PLANO_AGENTE
title: '[ZUNO-000] '
labels: needs-triage
---

## Objetivo

<!-- Resultado observável esperado em uma ou duas frases. -->

## Escopo / fora de escopo

- **Dentro do escopo:**
- **Fora do escopo (explícito):**

## Requisitos do PRD

<!-- Cite as seções/requisitos do PRD.md que esta tarefa atende. -->

## Interfaces e dados afetados

<!-- Endpoints, tipos compartilhados, migrations, arquivos, eventos. -->

## Dependências / riscos

<!-- Dependências de outros milestones, decisões pendentes (ex.: licença), riscos técnicos. -->

## Implementação proposta

<!-- Passos curtos, arquivos principais, alternativas consideradas. -->

## Critérios de aceite observáveis

- [ ] Comportamento: o caminho feliz produz o resultado descrito no objetivo.
- [ ] Erros: entradas inválidas retornam erro documentado (`{ "code", "message" }`), sem panic.
- [ ] Segurança: bind local por padrão, autorização/ownership verificados, nenhum segredo no código.
- [ ] Testes: cobertura automatizada da lógica nova; comandos executados no PR.
- [ ] Documentação: docs/README/ADR atualizados quando o comportamento muda.

## Evidências de validação

<!-- Saída real de cargo test, npm test, curl /health, etc. Sem invenções. -->

## Rollback (se altera DB/arquivos)

<!-- Migration reversível, backup, como reverter sem apagar músicas do usuário. -->

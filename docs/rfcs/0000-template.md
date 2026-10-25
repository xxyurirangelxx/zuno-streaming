# RFC-NNNN — Título curto e descritivo

- **Status:** Proposed
- **Milestone:** M0 / M1 / …
- **Autores:** @usuario
- **Data:** AAAA-MM-DD
- **Issues/PRs relacionados:** #000
- **ADRs resultantes:** ADR-000X (se aplicável)

## 1. Resumo

Um parágrafo explicando a proposta e o resultado esperado.

## 2. Motivação

Qual problema real existe hoje? Quais requisitos do `PRD.md` e do `PLANO_AGENTE.md` são atendidos?
Cite as seções. Se a proposta depende de uma decisão pendente do PRD, diga qual e não a tome por conta própria.

## 3. Escopo

**Dentro do escopo**

- …

**Fora do escopo**

- …

## 4. Proposta

Descreva a solução técnica: interfaces, dados, migrations, endpoints, componentes de UI, fluxos e erros.
Inclua diagramas (Mermaid) quando ajudar.

## 5. Alternativas consideradas

| Alternativa | Prós | Contras | Motivo de não ser escolhida |
|---|---|---|---|
| A | | | |
| B | | | |

## 6. Impacto

- **Dados/migrations:** (novo schema? rollback? backup?)
- **Segurança e privacidade:** (RBAC, ownership, dados pessoais, logs)
- **Desempenho em Raspberry Pi 3:** (memória, CPU, I/O)
- **Compatibilidade:** (API versionada, clientes existentes, migração de schema)
- **Operação:** (configuração, systemd/Docker, diagnóstico)

## 7. Riscos e mitigação

| Risco | Severidade | Mitigação |
|---|---|---|
| | | |

## 8. Plano de validação

Como saberemos que funcionou? Testes automatizados, testes manuais, matriz de plataformas, benchmarks reais.
Nada de “parece funcionar”.

## 9. Critérios de aceite observáveis

- [ ] Comportamento positivo
- [ ] Erros e fallback documentados
- [ ] Segurança e permissões cobertas por teste
- [ ] Testes automatizados
- [ ] Documentação, ADR e status do milestone atualizados

## 10. Rollback

Como desfazer com segurança (dados, arquivos, schema, configuração).

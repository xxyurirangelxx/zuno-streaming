# ADR-0006 — Licença open-source pendente

- **Status:** Proposed (não decidida)
- **Data:** 2026-10-25
- **Decisores:** proprietário do projeto
- **Fonte:** PRD §2.1 e §17; PLANO_AGENTE §8

## Contexto

O Zuno é descrito como open-source, mas a licença de reutilização **ainda não foi escolhida**. Publicar código sem `LICENSE` significa que, por padrão, todos os direitos são reservados: ninguém pode copiar, modificar ou redistribuir legalmente, mesmo que o código esteja público. Por outro lado, escolher uma licença sem avaliar as consequências (adoção, copyleft, compatibilidade de dependências, expectativa da comunidade) seria uma decisão irreversível tomada por engano.

O PRD registra a comparação abaixo como base para a decisão. **Não é aconselhamento jurídico.**

| Licença | Redistribuição proprietária | Obrigação de publicar alterações | Adequação ao Zuno |
|---|---|---|---|
| MIT | Permite | Não | Máxima adoção, menor reciprocidade |
| Apache-2.0 | Permite | Não | Permissiva + concessão expressa de patentes; boa para plugins/ecossistema |
| GPL-3.0 | Condicionada às obrigações de copyleft | Ao distribuir programa derivado | Protege redistribuições de clientes/servidor; não exige publicação só por hospedar serviço |
| AGPL-3.0 | Condicionada ao copyleft | Inclui usuários interagindo com programa modificado pela rede (na hipótese aplicável) | Mais forte para servidor self-hosted aberto, pode afastar alguns integradores |

**[PROPOSTO] no PRD:** Apache-2.0 se a prioridade for adoção/reuso/integrações; AGPL-3.0 se a prioridade for manter abertas alterações de servidores públicos.

## Decisão

**Nenhuma licença é declarada neste momento.**

- Não criar arquivo `LICENSE`.
- Não afirmar MIT, Apache-2.0, GPL-3.0, AGPL-3.0 nem qualquer outra licença em README, site, metadados ou releases.
- `package.json` permanece com `"license": "UNLICENSED"` e `"private": true`; o Cargo workspace permanece `publish = false`.
- O repositório pode ser público para desenvolvimento e revisão, mas **contribuições externas significativas não devem ser aceitas** até a decisão.
- A escolha final cabe ao proprietário e deve ser registrada nesta ADR (status `Accepted`) ou em uma ADR que a substitua, seguida da criação do `LICENSE` e da atualização dos metadados.

## Alternativas consideradas

| Alternativa | Prós | Contras |
|---|---|---|
| MIT | simples, adoção máxima | sem concessão explícita de patentes, sem reciprocidade |
| Apache-2.0 | permissiva, patentes explícitas, boa para ecossistema | permite fork fechado; requer avisos |
| GPL-3.0 | garante abertura de derivados distribuídos | não cobre uso como serviço; atrito para alguns integradores |
| AGPL-3.0 | fecha a brecha de SaaS; forte para self-hosted | afasta empresas; compatibilidade a verificar |
| Sem licença (atual) | nenhuma decisão precipitada | tecnicamente "todos os direitos reservados"; impede reuso |

## Consequências

- O repositório é público, mas **não é open-source licenciado** enquanto não houver `LICENSE`.
- Sem contribuições externas relevantes, o projeto perde colaboração e alcance.
- Dependências devem ser compatíveis com a licença futura; a auditoria precisa considerar essa escolha.
- Releases e binários não podem declarar uma licença antes da decisão.
- A situação deve ser comunicada com clareza em README, CONTRIBUTING e SECURITY para evitar uso indevido por terceiros.

## Próximos passos

1. Proprietário decide entre as opções (ou outra).
2. Verificar compatibilidade com dependências e contribuições existentes.
3. Criar `LICENSE`, atualizar `package.json`/metadados e esta ADR para `Accepted`.
4. Revisar política de contribuição e releases.

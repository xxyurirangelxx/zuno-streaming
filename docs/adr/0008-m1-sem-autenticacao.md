# ADR-0008 — M1 sem autenticação (bind loopback, autorização a partir do M2)

- **Status:** Accepted (temporária — expira no M2)
- **Data:** 2026-10-25
- **Decisores:** proprietário do projeto
- **Fonte:** PRD §2.5, §3 (F-01, F-04), §7; PLANO_AGENTE M1 e M2

## Contexto

O PRD classifica como P0 o streaming **autenticado** (F-04) e exige autorização em todo endpoint (§7), mas o
cronograma coloca autenticação, convites e RBAC no **M2**. O M1 precisa entregar scan, tags, busca e streaming com
`Range` antes disso, o que cria uma janela em que existe um endpoint de áudio sem controle de acesso.

Implementar uma “auth mínima” no M1 foi considerado e recusado: exigiria setup, tokens, revogação e testes de IDOR
antecipados, criando retrabalho quando o RBAC real chegar.

## Decisão

1. No M1 o servidor **não expõe autenticação** e escuta **apenas em loopback** (`ZUNO_BIND_ADDR=127.0.0.1:8080` por padrão).
2. Endpoints de leitura/streaming do M1 são **sem auth**, explicitamente documentados como limitação temporária.
3. **Nenhuma exposição de rede é permitida** enquanto não houver autenticação: mudar o bind para `0.0.0.0` no M1 é
   configuração insegura e deve ser tratada como não suportada (README, SECURITY e docs de instalação avisam).
4. O M2 deve, antes de habilitar qualquer acesso remoto:
   - implementar login Argon2id, sessão revogável e RBAC com defaults restritivos;
   - aplicar middleware de autorização em **todos** os endpoints, incluindo streaming;
   - adicionar testes de negação (401/403/404 sem vazamento de existência) e de isolamento entre usuários;
   - atualizar esta ADR para `Superseded by ADR-XXXX` e remover a ressalva dos docs.
5. Testes automatizados do M1 documentam o comportamento atual (sem auth) para que a mudança do M2 seja explícita.

## Alternativas consideradas

| Alternativa | Prós | Contras | Motivo da recusa |
|---|---|---|---|
| Auth mínima já no M1 | mais seguro desde o início | antecipa M2, retrabalho com RBAC real | escopo e cronograma |
| Token estático em variável de ambiente | simples | vira credencial compartilhada, sem revogação/auditoria | falsa sensação de segurança |
| Sem auth e bind `0.0.0.0` | acesso fácil na LAN | qualquer dispositivo lê a biblioteca | inseguro; recusado |

## Consequências

- O M1 é utilizável apenas na máquina local (ou via túnel SSH feito pelo próprio operador, fora do escopo do produto).
- Existe risco residual aceito e documentado: quem tem acesso ao loopback lê a biblioteca.
- O gate do M1 não pode declarar “streaming autenticado”; esse critério é transferido para o M2.
- Facilita a validação de `Range`/seek sem construir auth antes da hora.

## Status e próximos passos

- **Vigente apenas no M1.** Revisar obrigatoriamente ao iniciar o M2; a ADR do M2 substitui esta.

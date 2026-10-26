# Marco M2d — Upload na interface

Quarta entrega do M2: a web passa a enviar Opus para a biblioteca, com fila, progresso e tratamento de duplicata.

## Escopo entregue

- Rota “Enviar” no shell, com dropzone acessível (input múltiplo, drag-and-drop, teclado, `aria-live`).
- Fila **sequencial** com estados: aguardando, enviando (%), validando, indexado, duplicado, erro, cancelado.
- Progresso real via `XMLHttpRequest` (`withCredentials`, `X-Zuno-Filename`, `application/octet-stream`).
- Rejeição client-side de não-Opus com aviso de que a conversão é do cliente Windows/Tauri (M4, ADR-0007).
- Duplicata (`409`) mostra “já existe na biblioteca” com link para a faixa; nada é sobrescrito.
- Erros acionáveis para `413` (tamanho), `415`/`422` (formato), `401` (volta ao login) e rede (retry).
- Cancelamento aborta o envio e remove o staging (`DELETE`); retry reenvia o mesmo arquivo.
- Sucesso invalida a biblioteca (`libraryRevision` no `AuthProvider`) e oferece “Ir para a biblioteca”.

## Evidências de execução

Windows 10, Node 24, 2026-10-26.

```text
$ npm run typecheck        -> OK
$ npm run lint             -> 0 erros, 0 warnings
$ npm test                 -> 78 testes (50 web + 28 api-client)
$ npm run build            -> JS 314,36 kB (gzip 90,99 kB); CSS 35,72 kB
$ npx prettier --check .   -> OK
```

O backend de upload já havia sido validado em M1c (staging, 413/415/422, duplicata 409, movimentação atômica).

## Fora do M2d

- Conversão para Opus no navegador (não prevista; M4 no Windows).
- Upload resumível em blocos.
- Revisão de duplicatas em lote (hoje uma por arquivo).
- E2E em navegador real (M2e).

## Pendências

- OpenAPI agora documenta `/api/v1/uploads` (versão 0.5.0).
- Teste manual de upload pela UI em navegador real pendente.

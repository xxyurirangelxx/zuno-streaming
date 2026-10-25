# Matriz de capacidades de áudio e plataforma

Este documento registra **o que cada alvo realmente faz**, não o que gostaríamos que fizesse. A matriz é a fonte de verdade usada pela UI para decidir se um recurso é exibido, degradado ou oculto. Ver [ADR-0003](../adr/0003-arquitetura-de-clientes.md).

**Estado atual:** todas as células estão em `not-tested`. Nada foi implementado, medido ou validado fora do M0 (healthcheck e página de conectividade). O player, os adaptadores e os apps Tauri pertencem a M3+.

## 1. Estados

| Estado | Significado | Pode aparecer na UI? |
|---|---|---|
| `supported` | Comportamento verificado com evidência registrada (versão, data, teste) | sim, como recurso disponível |
| `limited` | Funciona com restrição conhecida e documentada (ex.: seek só dentro do buffer, sem gapless real) | sim, com aviso da limitação |
| `unsupported` | Verificado como indisponível no alvo | não; a UI esconde ou desabilita com explicação |
| `not-tested` | Sem evidência. Estado inicial de toda célula | não; tratar como indisponível na UI |

## 2. Regra de evidência (obrigatória)

1. Uma célula só sai de `not-tested` quando existir uma **entrada no log de evidências** (seção 4) com: recurso, alvo, versão do alvo, versão do Zuno/commit, data, procedimento e resultado observado.
2. `supported` exige teste **no alvo real** (motor de áudio/OS/browser), não em emulador, nem por inferência de documentação de terceiros.
3. Teste automatizado que apenas verifica presença de API **não** promove a célula para `supported`; no máximo para `limited`, e somente se o comportamento de áudio também foi observado.
4. Regressão descoberta rebaixa a célula imediatamente para `limited` ou `unsupported` e adiciona nota com a versão afetada.
5. Nunca copiar resultados entre colunas: Edge, Chrome e Firefox têm motores distintos; Windows (Tauri/WebView2) e Android (Tauri/WebView) não herdam o resultado do navegador.
6. Proibido preencher com "provavelmente", "deve funcionar" ou benchmark inventado. Sem medição real, o campo de observação fica vazio.

## 3. Matriz

Colunas = alvos planejados; linhas = capacidades. Atualizado pela última vez em 2026-10-25 (M0), sem nenhum teste executado.

| Capacidade | Windows (Tauri 2) | Edge | Chrome | Firefox | Android (Tauri 2) |
|---|---|---|---|---|---|
| `play` | not-tested | not-tested | not-tested | not-tested | not-tested |
| `seek` | not-tested | not-tested | not-tested | not-tested | not-tested |
| `gapless` | not-tested | not-tested | not-tested | not-tested | not-tested |
| `crossfade` | not-tested | not-tested | not-tested | not-tested | not-tested |
| `eq` | not-tested | not-tested | not-tested | not-tested | not-tested |
| `replaygain` | not-tested | not-tested | not-tested | not-tested | not-tested |
| `background` | not-tested | not-tested | not-tested | not-tested | not-tested |
| `media_controls` | not-tested | not-tested | not-tested | not-tested | not-tested |
| `offline` | not-tested | not-tested | not-tested | not-tested | not-tested |
| `export` | not-tested | not-tested | not-tested | not-tested | not-tested |
| `auto_update` | not-tested | not-tested | not-tested | not-tested | not-tested |

### 3.1 Glossário das capacidades

| Capacidade | O que significa |
|---|---|
| `play` | Iniciar reprodução de um arquivo servido pelo backend (M1+) |
| `seek` | Reposicionar dentro da faixa com `Range`/bufferização funcionando |
| `gapless` | Transição sem silêncio audível entre faixas consecutivas |
| `crossfade` | Sobreposição gradual entre faixas com duração configurável |
| `eq` | Equalizador (bandas/ganho) aplicado no cliente |
| `replaygain` | Normalização de volume por faixa/álbum |
| `background` | Reprodução continua com app em segundo plano ou tela bloqueada |
| `media_controls` | Integração com controles de mídia do sistema/notificação/teclas |
| `offline` | Reprodução de faixas baixadas sem rede |
| `export` | Exportar dados do usuário (playlists, histórico) em formato aberto |
| `auto_update` | Atualização do aplicativo com verificação de assinatura/checksum |

### 3.2 Observações por alvo

Nenhuma observação registrada. Quando houver, use o formato do log abaixo.

## 4. Log de evidências

Toda mudança de estado entra aqui (mais recente no topo). Campos vazios significam "não medido".

| Data | Capacidade | Alvo | Versão do alvo | Zuno (commit) | Procedimento | Resultado |
|---|---|---|---|---|---|---|
| — | — | — | — | — | — | nenhum teste executado |

### 4.1 Modelo de registro

```text
Data:            AAAA-MM-DD
Capacidade:      play | seek | gapless | crossfade | eq | replaygain |
                 background | media_controls | offline | export | auto_update
Alvo:            Windows (Tauri 2) | Edge | Chrome | Firefox | Android (Tauri 2)
Versão do alvo:  ex.: WebView2 1xx, Edge 1xx, Android 1x
Zuno:            commit/versão
Procedimento:    passos objetivos (arquivo, duração, ação, condição de rede)
Observado:       o que aconteceu (som, silêncio, erro, log)
Estado:          supported | limited | unsupported
Limitação:       obrigatório quando limited
```

## 5. Como testar (quando o player existir, M3+)

Procedimentos planejados — ainda **não** executados:

1. **`play`/`seek`**: faixa Opus longa (> 5 min); pausar, buscar para 80%, confirmar continuidade e tempo exibido. (A biblioteca é somente Opus desde a ADR-0007.)
2. **`gapless`**: álbum com faixas emendadas (`track_no` contínuo, sem silêncio no arquivo); confirmar ausência de pausa audível.
3. **`crossfade`**: 2 faixas com crossfade de 3 s; verificar sobreposição e ausência de clipping.
4. **`eq`/`replaygain`**: comparar com e sem o recurso; confirmar que o processamento não distorce e que o ganho é reversível.
5. **`background`/`media_controls`**: bloquear tela (Android) / minimizar (Windows) e usar teclas de mídia; observar se o áudio continua e se os controles refletem o estado.
6. **`offline`**: baixar faixa, desligar rede, reproduzir; verificar assinatura/validade do cache e limite de armazenamento.
7. **`export`/`auto_update`**: gerar export e reimportar; verificar atualização com checksum/assinatura e rollback.

Falhas e degradações devem aparecer para o usuário (a UI nunca deve esconder um recurso ausente), conforme [ADR-0003](../adr/0003-arquitetura-de-clientes.md).

## 6. Referências

- [PRD](../../PRD.md) §2.3 (reprodução e clientes) e §10 (requisitos não funcionais)
- [PLANO_AGENTE](../../PLANO_AGENTE.md) §6 (matriz de capacidades)
- [ADR-0003 — Arquitetura de clientes](../adr/0003-arquitetura-de-clientes.md)
- [Visão de arquitetura](overview.md)

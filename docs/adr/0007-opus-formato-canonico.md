# ADR-0007 — Opus 128 kbps VBR como formato canônico e conversão no cliente

- **Status:** Accepted
- **Data:** 2026-10-25
- **Decisores:** proprietário do projeto
- **Fonte:** PRD §2.2, §5.1, §5.2 e §11; PLANO_AGENTE M1 e M4

## Contexto

O PRD v2.0 aceitava MP3, FLAC, M4A/AAC, OGG/Vorbis e Opus, com Opus 128 kbps VBR como “preset para novas conversões”
e política “original-first, sem recodificar por padrão”. Isso deixava duas tensões em aberto:

1. Servir formatos heterogêneos no Raspberry Pi 3 sem transcodificar exige que o cliente suporte todos os codecs —
   paridade que o PRD não garante (matriz de capacidades).
2. Transcodificar no Pi 3 é caro (CPU/RAM) e o PRD proíbe transcodificação obrigatória em tempo real.

Em 25/10/2026 o proprietário decidiu que **todas as músicas do Zuno serão Opus 128 kbps VBR** e que a conversão
acontece **no cliente que envia**, não no servidor.

## Decisão

1. **Formato canônico da biblioteca: Opus 128 kbps VBR.** O servidor armazena e transmite apenas arquivos Opus.
2. **O servidor não converte nem transcodifica** — em nenhum milestone. Não existe fila de conversão no servidor.
3. **A conversão é responsabilidade do cliente** que envia os arquivos:
   - **Windows/Tauri (M4)** converte MP3/FLAC/M4A/AAC/OGG → Opus 128 kbps VBR com ffmpeg embutido antes do upload,
     preservando o original no dispositivo do usuário;
   - **Web (M2)** envia apenas Opus já pronto (converter no navegador não faz parte do escopo atual);
   - Android (M5) segue a mesma regra do cliente Windows quando implementado.
4. **Arquivos não-Opus no diretório monitorado** são ignorados e listados em relatório de scan
   (“formato não suportado: converta no cliente”). **Nunca são movidos nem apagados.**
5. **Upload de não-Opus é rejeitado** com erro claro (`415`, código `unsupported_format`).
6. **Originais permanecem no cliente.** O servidor não recebe, não arquiva e não exclui arquivos originais.
7. Fontes lossy (MP3/AAC) reencodadas para Opus têm perda de geração: o cliente deve avisar explicitamente antes de converter.
8. A taxa é **128 kbps VBR** por padrão; arquivos Opus com outra taxa são aceitos e reportados com a taxa real
   (sem reencode pelo servidor).

## Alternativas consideradas

| Alternativa | Prós | Contras | Motivo da recusa |
|---|---|---|---|
| Converter no servidor, na importação | funciona sem app desktop | CPU/RAM no Pi 3, fila complexa, contradiz “não transcodificar” | decisão do proprietário: conversão no cliente |
| Worker externo no PC converte e envia | tira carga do Pi | exige worker, autenticação e contrato extra desde o M1 | simplificado: o próprio cliente faz o trabalho |
| Conversão sob demanda no primeiro play | nada a fazer na importação | espera no primeiro acesso, CPU no caminho crítico | pior experiência e pior para o Pi |
| Manter multi-formato (PRD original) | máxima compatibilidade de entrada | exige codecs/paridade no cliente, testes por formato | decisão do proprietário por biblioteca homogênea |

## Consequências

- **Positivas:** biblioteca homogênea; streaming trivial (sem transcode); menos CPU no Pi; menos casos de
  incompatibilidade no cliente; tamanho previsível da biblioteca (~128 kbps).
- **Negativas:** o fluxo “importar minha biblioteca MP3/FLAC” só funciona a partir do M4 (Windows/Tauri);
  entre M1 e M3 a biblioteca precisa ser preparada externamente. Usuários de FLAC perdem qualidade (lossy) —
  o original fica no cliente, então nada é destruído, mas a biblioteca servida é lossy.
- **Técnicas:** o scanner filtra por contentor/codec Opus (`.opus`/`.ogg` com codec Opus), valida via probe e
  armazena `codec`/`bitrate` reais; testes usam fixtures Opus sintéticas geradas com ffmpeg.
- **Documentação:** PRD §2.2/§5.1/§5.2/§11, PLANO_AGENTE (M1 e M4) e README foram atualizados nesta mesma data.

## Status e próximos passos

- **Vigente.** Revisar se: (a) o M4 for adiado e a importação precisar existir antes; (b) o cliente web passar a
  converter no navegador; (c) surgir requisito de qualidade sem perdas na biblioteca (ex.: Opus 256 ou FLAC opcional).

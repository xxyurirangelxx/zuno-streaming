# ADR-0009 — Dependências do servidor musical e identificadores UUIDv7

- **Status:** Accepted
- **Data:** 2026-10-25
- **Decisores:** proprietário do projeto
- **Fonte:** PRD §5.3; PLANO_AGENTE M1; RFC-0001

## Contexto

O M1a introduz leitura de tags, hash de arquivos, caminhada de diretórios, sniffing de conteúdo e identificadores
estáveis. O M0 tinha apenas `axum`, `tokio`, `sqlx`, `serde`, `tracing` e utilitários de teste. Era preciso decidir
quais bibliotecas entram — mantendo o consumo baixo no Raspberry Pi 3 (armv7) — e como identificar entidades sem
depender de caminhos de arquivo.

## Decisão

**Identificadores:** UUIDv7 em `TEXT` como chave primária de artistas, álbuns, faixas, arquivos, capas e execuções de
scan. UUIDv7 é ordenável por tempo (bom para índices B-tree no SQLite) e não enumerável como IDs sequenciais.
Caminho de arquivo continua sendo atributo mutável (`audio_files.relative_path UNIQUE`), nunca identidade.

**Dependências adicionadas** (todas MIT/Apache-2.0, compatíveis com a licença do projeto):

| Crate | Versão | Para quê | Por que não escrever à mão |
|---|---|---|---|
| `lofty` | 0.25 | Ler tags, duração, propriedades e capa embutida (Opus/Vorbis) | parser de Ogg/Opus + Vorbis comments + METADATA_BLOCK_PICTURE é grande e cheio de casos de borda |
| `walkdir` | 2.5 | Caminhar a biblioteca sem seguir symlinks, com erros por entrada | evita reimplementar recursão com tratamento de erro e limites de profundidade |
| `sha2` | 0.10 | sha256 dos arquivos (deduplicação exata, checksum de capa) | implementação de criptografia não deve ser caseira |
| `infer` | 0.22 | Sniff de contêiner/imagem por magic bytes | tabela de assinaturas é mantida por terceiros e testada |
| `uuid` | 1 (features `v7`, `serde`) | Geração de UUIDv7 | geração monotônica correta do v7 não é trivial |

Nenhuma dependência exige serviço externo, rede em runtime, telemetria ou licença paga. `lofty` e `infer` são
pequenos; `sha2` é usado apenas no scan, fora do caminho de streaming.

**Não adicionadas nesta fase:** `image` (resize de capa fica para o M1b), `tokio-util` (streaming no M1b),
`clap` (CLI é um único subcomando; `std::env::args` basta), crate de data (`strftime` do SQLite cobre).

## Alternativas consideradas

| Alternativa | Prós | Contras | Motivo da recusa |
|---|---|---|---|
| IDs `INTEGER PRIMARY KEY` | menores, rápidos | enumeráveis; colidem entre importações/dispositivos | UUIDv7 mantém estabilidade e não vaza volume |
| ULID | ordenável, compacto | mais uma dependência para ganho marginal sobre UUIDv7 | UUIDv7 já resolve |
| Hash do caminho como ID | zero dependência | muda ao renomear; colide em conteúdo idêntico | viola “IDs independentes de caminho” |
| Parser de Opus próprio | zero dependência | alto risco de corrupção silenciosa | `lofty` é maduro e testado |
| ffprobe externo no scan | metadados ricos | exige binário externo no Pi, mais um processo por arquivo | adiado; conversão no cliente já usa ffmpeg (M4) |

## Consequências

- **Positivas:** scan confiável e idempotente; IDs estáveis para sync futuro; dependências auditáveis e pequenas.
- **Negativas:** mais cinco crates no binário (impacto modesto: `cargo build --release` e `cargo audit` medidos no PR);
  UUID em `TEXT` ocupa mais espaço que inteiro (irrelevante para 1.000 faixas).
- **Operacionais:** `cargo audit` cobre as novas crates; nenhuma delas aparece na exceção documentada
  (RUSTSEC-2023-0071, `rsa`, fora do grafo de build).
- **Revisão:** reavaliar se o número de dependências crescer sem necessidade comprovada, ou se o scan no Pi 3
  mostrar que `lofty` é o gargalo.

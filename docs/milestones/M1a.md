# Marco M1a — Domínio e scanner da biblioteca

Registro do primeiro terço do M1: modelo de dados musical, extração de metadados Opus e scanner incremental.
Streaming (M1b) e upload (M1c) **não** fazem parte desta entrega.

## Objetivo

Indexar uma biblioteca **somente Opus** sem mover, renomear ou apagar nada, com varredura idempotente,
tolerante a arquivos ausentes/corrompidos e com relatório auditável.

## Escopo entregue

### Banco (migration `0002_music_domain.sql`)

- `artists`, `albums`, `tracks`, `track_artists`, `artworks`, `audio_files`.
- `tracks_fts` (FTS5, `unicode61 remove_diacritics 2`) mantido pela aplicação.
- `scan_runs` e `scan_issues` para auditoria de cada varredura.
- Índices em álbum, título normalizado, sha256, disponibilidade e artistas.
- IDs **UUIDv7** em `TEXT` ([ADR-0009](../adr/0009-dependencias-e-ids-m1.md)); caminho é atributo mutável.

### Scanner (`zuno-server scan`)

- Caminhada com `walkdir` **sem seguir symlinks**; `canonicalize` + verificação de prefixo contra a raiz.
- Filtro de extensão (`.opus`/`.ogg`) e confirmação de codec **Opus** via `lofty`.
- Limite de tamanho por arquivo (`ZUNO_MAX_AUDIO_FILE_BYTES`, 1 KiB–10 GiB).
- Tags (título, artistas, album artist, álbum, faixa, disco, ano), duração, bitrate, sample rate e canais.
- Fallback de título pelo nome do arquivo quando não há tag.
- Capas embutidas extraídas para `ZUNO_ARTWORK_DIR` com nome = sha256 e escrita atômica (`.tmp` + rename).
- `sha256` em blocos de 64 KiB; **rehash evitado** quando tamanho e mtime não mudaram.
- Duplicata exata (mesmo sha256) é reportada (`duplicate_exact`) e **ambos os arquivos permanecem**.
- Arquivo ausente vira `available = 0`; **nenhuma linha é apagada**.
- Relatório JSON em stdout + registro em `scan_runs`/`scan_issues`.
- `ZUNO_SCAN_ON_STARTUP=true` executa o scan antes de subir o servidor HTTP.

### Dependências

`lofty` 0.25, `walkdir` 2.5, `sha2` 0.10, `infer` 0.22, `uuid` 1 (v7) — justificadas em
[ADR-0009](../adr/0009-dependencias-e-ids-m1.md). Nenhuma exige rede ou serviço externo.

## Evidências de execução

Windows 10, Rust 1.99 (MSVC), 2026-10-25, a partir da raiz do repositório.

```text
$ cargo fmt --all --check
(sem saída; exit 0)

$ cargo clippy -p zuno-server --all-targets -- -D warnings
Finished `dev` profile ... (0 warnings)

$ cargo test -p zuno-server
lib          21 passed
main          3 passed
health        3 passed
migrations    3 passed
metadata      7 passed
repository   13 passed
scanner       6 passed
doc-tests     0 passed
total        56 passed; 0 failed
```

Smoke test real do binário (biblioteca temporária com as 6 fixtures):

```text
$ zuno-server scan            # 1ª execução
files_seen=6 files_indexed=4 files_unchanged=0 files_skipped=1 files_failed=1 files_missing=0
issues: corrupt_file (broken.opus), unsupported_format (vorbis.ogg), duplicate_exact (par byte-idêntico)

$ zuno-server scan            # 2ª execução (idempotente)
files_seen=6 files_indexed=0 files_unchanged=4 ...

$ rm tone-b.opus && zuno-server scan
files_seen=5 files_indexed=0 files_unchanged=3 files_missing=1
```

O cache de capas recebeu `cc6a30a1….png` (sha256 do conteúdo) **fora** da biblioteca, e a árvore da biblioteca
permaneceu inalterada (nenhum arquivo criado, movido ou removido pelo Zuno).

### Fixtures

`fixtures/audio/` (122 KB no total, versionado) gerado por `scripts/generate-fixtures.py` com ffmpeg:
`tone-a.opus` (tags + capa embutida), `tone-b.opus`, `no-tags.opus`, `duplicate-a.opus`, `vorbis.ogg`,
`broken.opus`. Nenhum áudio de terceiros é usado.

## Fora do M1a

- Endpoints de listagem/busca/stream (M1b).
- Redimensionamento de capas (M1b) — hoje o cache guarda os bytes originais.
- Upload/staging e rejeição `415` de não-Opus (M1c).
- Autenticação (M2, [ADR-0008](../adr/0008-m1-sem-autenticacao.md)).
- Medição em Raspberry Pi 3 B (armv7): hardware ainda não disponível.
- Escrita de tags, organização automática de arquivos (M8/fora de escopo).

## Pendências conhecidas

- Gate do M1 só fecha com M1b + M1c e com a medição no Pi 3 B.
- Nenhum índice de FTS para artistas/álbuns separado: a busca cobre título, álbum e artistas por faixa.
- Capas de `folder.jpg`/`cover.jpg` não são lidas (apenas embutidas) — decidir no M1b.

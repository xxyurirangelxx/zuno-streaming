# ADR-0004 — Armazenamento de áudio no filesystem

- **Status:** Accepted
- **Data:** 2026-10-25
- **Decisores:** proprietário do projeto (via PRD v2.0 e `PLANO_AGENTE.md`)
- **Fonte:** PRD §2.2, §5.1–§5.3; PLANO_AGENTE §1.3

## Contexto

A biblioteca pode ter até 1.000 músicas no dimensionamento inicial, com arquivos MP3/FLAC/M4A/OGG de tamanhos variados em microSD. Guardar bytes de áudio no banco inflaria o SQLite, prejudicaria WAL/backup e dificultaria streaming com `Range`. Ao mesmo tempo, caminhos de arquivo mudam (reorganização, montagem de disco, Windows vs Linux), então não podem ser a identidade de uma faixa.

## Decisão

- **Bytes de áudio ficam no filesystem**, em diretório configurado (`ZUNO_MUSIC_DIR`, reservado ao M1). **Nunca** no SQLite.
- **Metadados e índices ficam no SQLite** (faixas, artistas, álbuns, arquivos, capas, jobs).
- **Original-first:** o arquivo original é preservado por padrão; conversão (ex.: Opus 128 kbps VBR) é opt-in por importação e nunca recodifica silenciosamente um lossy.
- **IDs estáveis e independentes de caminho** (UUID/ULID ou equivalente) para faixas e arquivos. O caminho é atributo mutável, não chave.
- **`AudioStorage`** abstrai o acesso ao filesystem (local no MVP); `transcode`/`fingerprint` são workers opcionais.
- Escritas/movimentações relevantes devem ser atômicas e transacionais com o banco sempre que possível.
- Nunca excluir música sem confirmação explícita e autorização; arquivo ausente é sinalizado, não apagado do banco automaticamente.

No M0 não há áudio nem schema de domínio; a decisão orienta o M1.

## Alternativas consideradas

| Alternativa | Prós | Contras | Motivo da recusa |
|---|---|---|---|
| BLOBs no SQLite | backup único | banco enorme, WAL lento, Range ineficiente | inviável para streaming |
| Object storage (S3/MinIO) | escala, separação | serviço extra obrigatório, rede local | viola simplicidade/offline |
| Caminho como chave primária | simples | quebra ao mover/renomear/montar | frágil |
| Biblioteca gerenciada sempre | organização consistente | mover arquivos do usuário sem permissão | arriscado; modo somente leitura é proposto |

## Consequências

- Backup precisa cobrir banco **e** arquivos separadamente (áudio opcional).
- Verificação de existência/checksum é necessária para detectar arquivos removidos ou alterados.
- Segurança de caminhos (traversal, symlink escape) é obrigatória no staging/import (M1).
- Índices/FTS5 permanecem baratos; streaming lê blocos diretamente do disco.
- Mover a biblioteca exige reconciliar caminhos, não recriar identidades.

## Próximos passos

- Definir layout de diretórios e staging no M1.
- Especificar política de conversão, checksum e reconciliação no import.

//! Repositório SQLite: única camada que conhece SQL.
//!
//! Regras:
//! * nada de DDL em runtime (schema só via migrations);
//! * toda escrita relevante é transacional quando envolve mais de uma tabela;
//! * o FTS (`tracks_fts`) é mantido explicitamente pela aplicação;
//! * nunca apagar arquivos/faixas do usuário — ausência vira `available = 0`.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use sqlx::{QueryBuilder, SqliteConnection, SqlitePool};

use crate::domain::dto::StreamTarget;
use crate::domain::{AudioFileDraft, ScanIssue, ScanReport, TrackDraft, normalize_key};

pub mod library;
pub mod uploads;

/// Expressão SQLite de "agora" no formato ISO-8601 UTC (mesmo padrão das migrations).
const NOW_SQL: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

/// Limite de linhas por `INSERT` na tabela temporária de caminhos vistos.
const SEEN_PATHS_CHUNK: usize = 500;

/// Limite mínimo aceito em `search_tracks`.
const MIN_SEARCH_LIMIT: i64 = 1;
/// Limite máximo aceito em `search_tracks`.
const MAX_SEARCH_LIMIT: i64 = 100;

/// Estado resumido de um arquivo já indexado, usado para pular rehash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFileState {
    pub id: String,
    /// Faixa já associada a este arquivo (reutilizada em rescans).
    pub track_id: String,
    pub size_bytes: u64,
    pub source_mtime: i64,
    pub sha256: String,
    pub available: bool,
}

/// Contadores persistidos em `scan_runs`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanCounts {
    pub files_seen: u64,
    pub files_indexed: u64,
    pub files_unchanged: u64,
    pub files_skipped: u64,
    pub files_failed: u64,
    pub files_missing: u64,
}

impl From<&ScanReport> for ScanCounts {
    fn from(report: &ScanReport) -> Self {
        Self {
            files_seen: report.files_seen,
            files_indexed: report.files_indexed,
            files_unchanged: report.files_unchanged,
            files_skipped: report.files_skipped,
            files_failed: report.files_failed,
            files_missing: report.files_missing,
        }
    }
}

/// Referência de capa já gravada no cache local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkDraft {
    pub relative_path: String,
    pub source: String,
    pub mime_type: Option<String>,
    pub checksum: String,
}

/// Cria a linha de `scan_runs` com estado `running`.
pub async fn start_scan_run(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query(&format!(
        "INSERT INTO scan_runs (id, started_at, state) VALUES (?1, {NOW_SQL}, 'running')"
    ))
    .bind(id)
    .execute(pool)
    .await
    .with_context(|| format!("falha ao iniciar a varredura '{id}'"))?;

    Ok(())
}

/// Fecha a linha de `scan_runs` com contadores e estado final.
pub async fn finish_scan_run(
    pool: &SqlitePool,
    id: &str,
    counts: ScanCounts,
    error: Option<&str>,
) -> Result<()> {
    let state = if error.is_some() {
        "failed"
    } else {
        "completed"
    };

    let result = sqlx::query(&format!(
        "UPDATE scan_runs SET
             finished_at = {NOW_SQL},
             state = ?2,
             files_seen = ?3,
             files_indexed = ?4,
             files_unchanged = ?5,
             files_skipped = ?6,
             files_failed = ?7,
             files_missing = ?8,
             error = ?9
         WHERE id = ?1"
    ))
    .bind(id)
    .bind(state)
    .bind(as_i64(counts.files_seen)?)
    .bind(as_i64(counts.files_indexed)?)
    .bind(as_i64(counts.files_unchanged)?)
    .bind(as_i64(counts.files_skipped)?)
    .bind(as_i64(counts.files_failed)?)
    .bind(as_i64(counts.files_missing)?)
    .bind(error)
    .execute(pool)
    .await
    .with_context(|| format!("falha ao finalizar a varredura '{id}'"))?;

    if result.rows_affected() == 0 {
        bail!("varredura '{id}' não encontrada ao finalizar");
    }

    Ok(())
}

/// Persiste um problema de scan em `scan_issues`.
pub async fn record_scan_issue(
    pool: &SqlitePool,
    scan_run_id: &str,
    issue: &ScanIssue,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO scan_issues (scan_run_id, relative_path, code, detail)
         VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(scan_run_id)
    .bind(&issue.relative_path)
    .bind(issue.code.as_str())
    .bind(issue.detail.as_deref())
    .execute(pool)
    .await
    .with_context(|| {
        format!(
            "falha ao registrar o problema '{}' de '{}'",
            issue.code, issue.relative_path
        )
    })?;

    Ok(())
}

/// Carrega o estado de todos os arquivos indexados, indexado por caminho relativo.
pub async fn load_audio_file_states(pool: &SqlitePool) -> Result<HashMap<String, AudioFileState>> {
    let rows = sqlx::query_as::<_, (String, String, String, i64, i64, String, i64)>(
        "SELECT relative_path, id, track_id, size_bytes, source_mtime, sha256, available
         FROM audio_files",
    )
    .fetch_all(pool)
    .await
    .context("falha ao carregar o estado dos arquivos de áudio")?;

    let mut states = HashMap::with_capacity(rows.len());
    for (relative_path, id, track_id, size_bytes, source_mtime, sha256, available) in rows {
        states.insert(
            relative_path,
            AudioFileState {
                id,
                track_id,
                size_bytes: as_u64(size_bytes)?,
                source_mtime,
                sha256,
                available: available != 0,
            },
        );
    }

    Ok(states)
}

/// Retorna o `id` de um `audio_files` com o mesmo sha256 (para detectar duplicata exata).
pub async fn find_audio_file_by_sha(pool: &SqlitePool, sha256: &str) -> Result<Option<String>> {
    sqlx::query_scalar::<_, String>(
        "SELECT id FROM audio_files WHERE sha256 = ?1 ORDER BY available DESC, scanned_at LIMIT 1",
    )
    .bind(sha256)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("falha ao procurar o arquivo de áudio com sha256 '{sha256}'"))
}

/// Cria ou reutiliza um artista pelo nome (normalizado) e retorna o `id`.
pub async fn upsert_artist(pool: &SqlitePool, name: &str) -> Result<String> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;
    let id = upsert_artist_in(&mut tx, name).await?;
    tx.commit().await.context("falha ao confirmar transação")?;
    Ok(id)
}

/// Cria ou reutiliza um álbum e retorna o `id`.
pub async fn upsert_album(
    pool: &SqlitePool,
    title: &str,
    album_artist_id: Option<&str>,
    release_year: Option<i32>,
    artwork_id: Option<&str>,
) -> Result<String> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;
    let id = upsert_album_in(&mut tx, title, album_artist_id, release_year, artwork_id).await?;
    tx.commit().await.context("falha ao confirmar transação")?;
    Ok(id)
}

/// Insere ou atualiza a faixa, seus artistas e a entrada no FTS.
pub async fn upsert_track(pool: &SqlitePool, draft: &TrackDraft) -> Result<()> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;
    upsert_track_in(&mut tx, draft).await?;
    tx.commit().await.context("falha ao confirmar transação")?;
    Ok(())
}

/// Insere ou atualiza o arquivo de áudio (idempotente por `relative_path`).
pub async fn upsert_audio_file(pool: &SqlitePool, draft: &AudioFileDraft) -> Result<()> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;
    upsert_audio_file_in(&mut tx, draft).await?;
    tx.commit().await.context("falha ao confirmar transação")?;
    Ok(())
}

/// Marca como indisponíveis os arquivos não vistos neste scan. Nunca apaga linhas.
///
/// Retorna quantos registros passaram de `available = 1` para `0`.
pub async fn mark_missing_audio_files(pool: &SqlitePool, seen_paths: &[String]) -> Result<u64> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;

    // A tabela temporária vive na conexão da transação e é descartada no fim.
    sqlx::query("CREATE TEMP TABLE IF NOT EXISTS seen_paths (path TEXT PRIMARY KEY NOT NULL)")
        .execute(&mut *tx)
        .await
        .context("falha ao criar a tabela temporária de caminhos vistos")?;
    sqlx::query("DELETE FROM seen_paths")
        .execute(&mut *tx)
        .await
        .context("falha ao limpar a tabela temporária de caminhos vistos")?;

    for chunk in seen_paths.chunks(SEEN_PATHS_CHUNK) {
        let mut builder: QueryBuilder<'_, sqlx::Sqlite> =
            QueryBuilder::new("INSERT OR IGNORE INTO seen_paths (path) ");
        builder.push_values(chunk, |mut row, path| {
            row.push_bind(path);
        });
        builder
            .build()
            .execute(&mut *tx)
            .await
            .context("falha ao registrar os caminhos vistos")?;
    }

    let result = sqlx::query(
        "UPDATE audio_files SET available = 0
         WHERE available = 1
           AND NOT EXISTS (
               SELECT 1 FROM seen_paths s WHERE s.path = audio_files.relative_path
           )",
    )
    .execute(&mut *tx)
    .await
    .context("falha ao marcar os arquivos ausentes")?;

    sqlx::query("DROP TABLE IF EXISTS seen_paths")
        .execute(&mut *tx)
        .await
        .context("falha ao descartar a tabela temporária de caminhos vistos")?;

    let marked = result.rows_affected();
    tx.commit().await.context("falha ao confirmar transação")?;

    Ok(marked)
}

/// Busca faixas no FTS5 e retorna os `track_id` ordenados por relevância.
pub async fn search_tracks(pool: &SqlitePool, query: &str, limit: i64) -> Result<Vec<String>> {
    let Some(expression) = build_match_expression(query) else {
        return Ok(Vec::new());
    };

    let limit = limit.clamp(MIN_SEARCH_LIMIT, MAX_SEARCH_LIMIT);

    sqlx::query_scalar::<_, String>(
        "SELECT track_id FROM tracks_fts WHERE tracks_fts MATCH ?1 ORDER BY rank LIMIT ?2",
    )
    .bind(expression)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("falha ao buscar faixas no índice full-text")
}

/// Cria ou reutiliza uma capa pelo checksum e retorna o `id`.
pub async fn upsert_artwork(pool: &SqlitePool, draft: &ArtworkDraft) -> Result<String> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;
    let id = upsert_artwork_in(&mut tx, draft).await?;
    tx.commit().await.context("falha ao confirmar transação")?;
    Ok(id)
}

/// Persiste faixa + artistas + FTS + arquivo de áudio em uma única transação.
///
/// `artwork` é gravado antes e vinculado ao álbum (quando houver álbum).
pub async fn persist_track_with_file(
    pool: &SqlitePool,
    track: &TrackDraft,
    file: &AudioFileDraft,
    artwork: Option<&ArtworkDraft>,
) -> Result<()> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;

    if let Some(draft) = artwork {
        let artwork_id = upsert_artwork_in(&mut tx, draft).await?;
        if let Some(album_id) = track.album_id.as_deref() {
            link_album_artwork_in(&mut tx, album_id, &artwork_id).await?;
        }
    }

    upsert_track_in(&mut tx, track).await?;
    upsert_audio_file_in(&mut tx, file).await?;

    tx.commit().await.context("falha ao confirmar transação")
}

// ---------------------------------------------------------------------------
// Helpers privados (reutilizados dentro e fora de transações)
// ---------------------------------------------------------------------------

async fn upsert_artist_in(conn: &mut SqliteConnection, name: &str) -> Result<String> {
    let normalized = normalize_key(name);

    sqlx::query(
        "INSERT INTO artists (id, name, normalized_name) VALUES (?1, ?2, ?3)
         ON CONFLICT (normalized_name) DO NOTHING",
    )
    .bind(new_id())
    .bind(name)
    .bind(&normalized)
    .execute(&mut *conn)
    .await
    .with_context(|| format!("falha ao gravar o artista '{name}'"))?;

    sqlx::query_scalar::<_, String>("SELECT id FROM artists WHERE normalized_name = ?1")
        .bind(&normalized)
        .fetch_one(&mut *conn)
        .await
        .with_context(|| format!("falha ao recuperar o artista '{name}'"))
}

async fn upsert_album_in(
    conn: &mut SqliteConnection,
    title: &str,
    album_artist_id: Option<&str>,
    release_year: Option<i32>,
    artwork_id: Option<&str>,
) -> Result<String> {
    let normalized = normalize_key(title);

    // `IS` compara de forma null-safe, então álbuns sem artista principal
    // continuam idempotentes (a UNIQUE do schema não cobre `album_artist_id IS NULL`).
    let existing = sqlx::query_scalar::<_, String>(
        "SELECT id FROM albums WHERE normalized_title = ?1 AND album_artist_id IS ?2",
    )
    .bind(&normalized)
    .bind(album_artist_id)
    .fetch_optional(&mut *conn)
    .await
    .with_context(|| format!("falha ao procurar o álbum '{title}'"))?;

    if let Some(id) = existing {
        // Só preenche lacunas: nunca sobrescreve dados já gravados.
        sqlx::query(&format!(
            "UPDATE albums SET
                 artwork_id = COALESCE(artwork_id, ?2),
                 release_year = COALESCE(release_year, ?3),
                 updated_at = {NOW_SQL}
             WHERE id = ?1"
        ))
        .bind(&id)
        .bind(artwork_id)
        .bind(release_year)
        .execute(&mut *conn)
        .await
        .with_context(|| format!("falha ao atualizar o álbum '{title}'"))?;

        return Ok(id);
    }

    sqlx::query(
        "INSERT INTO albums (id, title, normalized_title, album_artist_id, release_year, artwork_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (album_artist_id, normalized_title) DO NOTHING",
    )
    .bind(new_id())
    .bind(title)
    .bind(&normalized)
    .bind(album_artist_id)
    .bind(release_year)
    .bind(artwork_id)
    .execute(&mut *conn)
    .await
    .with_context(|| format!("falha ao gravar o álbum '{title}'"))?;

    sqlx::query_scalar::<_, String>(
        "SELECT id FROM albums WHERE normalized_title = ?1 AND album_artist_id IS ?2",
    )
    .bind(&normalized)
    .bind(album_artist_id)
    .fetch_one(&mut *conn)
    .await
    .with_context(|| format!("falha ao recuperar o álbum '{title}'"))
}

async fn upsert_track_in(conn: &mut SqliteConnection, draft: &TrackDraft) -> Result<()> {
    sqlx::query(&format!(
        "INSERT INTO tracks (
             id, album_id, title, normalized_title, disc_no, track_no, duration_ms, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, {NOW_SQL})
         ON CONFLICT (id) DO UPDATE SET
             album_id = excluded.album_id,
             title = excluded.title,
             normalized_title = excluded.normalized_title,
             disc_no = excluded.disc_no,
             track_no = excluded.track_no,
             duration_ms = excluded.duration_ms,
             updated_at = {NOW_SQL}"
    ))
    .bind(&draft.id)
    .bind(draft.album_id.as_deref())
    .bind(&draft.title)
    .bind(&draft.normalized_title)
    .bind(optional_i64(draft.disc_no.map(u64::from))?)
    .bind(optional_i64(draft.track_no.map(u64::from))?)
    .bind(optional_i64(draft.duration_ms)?)
    .execute(&mut *conn)
    .await
    .with_context(|| format!("falha ao gravar a faixa '{}'", draft.title))?;

    sqlx::query("DELETE FROM track_artists WHERE track_id = ?1")
        .bind(&draft.id)
        .execute(&mut *conn)
        .await
        .with_context(|| format!("falha ao limpar os artistas da faixa '{}'", draft.title))?;

    for (artist_id, role, sort_order) in &draft.artists {
        sqlx::query(
            "INSERT OR REPLACE INTO track_artists (track_id, artist_id, role, sort_order)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&draft.id)
        .bind(artist_id)
        .bind(role)
        .bind(as_i64(u64::from(*sort_order))?)
        .execute(&mut *conn)
        .await
        .with_context(|| format!("falha ao vincular o artista '{artist_id}' à faixa"))?;
    }

    sync_track_fts_in(conn, &draft.id).await
}

async fn upsert_audio_file_in(conn: &mut SqliteConnection, draft: &AudioFileDraft) -> Result<()> {
    let updated = sqlx::query(&format!(
        "UPDATE audio_files SET
             track_id = ?2,
             sha256 = ?3,
             codec = ?4,
             container = ?5,
             bitrate = ?6,
             sample_rate = ?7,
             channels = ?8,
             size_bytes = ?9,
             source_mtime = ?10,
             available = ?11,
             scanned_at = {NOW_SQL}
         WHERE relative_path = ?1"
    ))
    .bind(&draft.relative_path)
    .bind(&draft.track_id)
    .bind(&draft.sha256)
    .bind(&draft.codec)
    .bind(&draft.container)
    .bind(optional_i64(draft.bitrate.map(u64::from))?)
    .bind(optional_i64(draft.sample_rate.map(u64::from))?)
    .bind(optional_i64(draft.channels.map(u64::from))?)
    .bind(as_i64(draft.size_bytes)?)
    .bind(draft.source_mtime)
    .bind(available_flag(draft.available))
    .execute(&mut *conn)
    .await
    .with_context(|| {
        format!(
            "falha ao atualizar o arquivo de áudio '{}'",
            draft.relative_path
        )
    })?;

    if updated.rows_affected() > 0 {
        return Ok(());
    }

    sqlx::query(&format!(
        "INSERT INTO audio_files (
             id, track_id, relative_path, sha256, codec, container, bitrate, sample_rate,
             channels, size_bytes, source_mtime, available, scanned_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, {NOW_SQL})
         ON CONFLICT (relative_path) DO UPDATE SET
             track_id = excluded.track_id,
             sha256 = excluded.sha256,
             codec = excluded.codec,
             container = excluded.container,
             bitrate = excluded.bitrate,
             sample_rate = excluded.sample_rate,
             channels = excluded.channels,
             size_bytes = excluded.size_bytes,
             source_mtime = excluded.source_mtime,
             available = excluded.available,
             scanned_at = {NOW_SQL}"
    ))
    .bind(&draft.id)
    .bind(&draft.track_id)
    .bind(&draft.relative_path)
    .bind(&draft.sha256)
    .bind(&draft.codec)
    .bind(&draft.container)
    .bind(optional_i64(draft.bitrate.map(u64::from))?)
    .bind(optional_i64(draft.sample_rate.map(u64::from))?)
    .bind(optional_i64(draft.channels.map(u64::from))?)
    .bind(as_i64(draft.size_bytes)?)
    .bind(draft.source_mtime)
    .bind(available_flag(draft.available))
    .execute(&mut *conn)
    .await
    .with_context(|| {
        format!(
            "falha ao gravar o arquivo de áudio '{}'",
            draft.relative_path
        )
    })?;

    Ok(())
}

async fn upsert_artwork_in(conn: &mut SqliteConnection, draft: &ArtworkDraft) -> Result<String> {
    if let Some(id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM artworks WHERE checksum = ?1 ORDER BY created_at, id LIMIT 1",
    )
    .bind(&draft.checksum)
    .fetch_optional(&mut *conn)
    .await
    .with_context(|| format!("falha ao procurar a capa '{}'", draft.relative_path))?
    {
        return Ok(id);
    }

    sqlx::query(
        "INSERT INTO artworks (id, relative_path, source, mime_type, checksum)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (relative_path) DO NOTHING",
    )
    .bind(new_id())
    .bind(&draft.relative_path)
    .bind(&draft.source)
    .bind(draft.mime_type.as_deref())
    .bind(&draft.checksum)
    .execute(&mut *conn)
    .await
    .with_context(|| format!("falha ao gravar a capa '{}'", draft.relative_path))?;

    sqlx::query_scalar::<_, String>(
        "SELECT id FROM artworks WHERE checksum = ?1 OR relative_path = ?2
         ORDER BY created_at, id LIMIT 1",
    )
    .bind(&draft.checksum)
    .bind(&draft.relative_path)
    .fetch_one(&mut *conn)
    .await
    .with_context(|| format!("falha ao recuperar a capa '{}'", draft.relative_path))
}

async fn link_album_artwork_in(
    conn: &mut SqliteConnection,
    album_id: &str,
    artwork_id: &str,
) -> Result<()> {
    sqlx::query(&format!(
        "UPDATE albums SET artwork_id = ?2, updated_at = {NOW_SQL}
         WHERE id = ?1 AND artwork_id IS NULL"
    ))
    .bind(album_id)
    .bind(artwork_id)
    .execute(&mut *conn)
    .await
    .with_context(|| format!("falha ao vincular a capa '{artwork_id}' ao álbum '{album_id}'"))?;

    Ok(())
}

/// Re-sincroniza a linha do FTS5 para a faixa indicada.
///
/// O FTS é derivado de três tabelas, então a aplicação sempre apaga antes de
/// inserir — nenhuma linha órfã fica para trás.
async fn sync_track_fts_in(conn: &mut SqliteConnection, track_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM tracks_fts WHERE track_id = ?1")
        .bind(track_id)
        .execute(&mut *conn)
        .await
        .with_context(|| format!("falha ao limpar o índice full-text da faixa '{track_id}'"))?;

    sqlx::query(
        "INSERT INTO tracks_fts (track_id, title, album, artists)
         SELECT t.id,
                t.title,
                COALESCE((SELECT al.title FROM albums al WHERE al.id = t.album_id), ''),
                COALESCE((
                    SELECT group_concat(ordered.name, ' ')
                    FROM (
                        SELECT ar.name AS name
                        FROM track_artists ta
                        JOIN artists ar ON ar.id = ta.artist_id
                        WHERE ta.track_id = t.id
                        ORDER BY ta.sort_order, ar.name
                    ) AS ordered
                ), '')
         FROM tracks t
         WHERE t.id = ?1",
    )
    .bind(track_id)
    .execute(&mut *conn)
    .await
    .with_context(|| format!("falha ao reindexar a faixa '{track_id}' no full-text"))?;

    Ok(())
}

/// Monta a expressão FTS5 a partir da consulta do usuário.
///
/// Mantém apenas letras, números e espaços (descarta a sintaxe do FTS5 para não
/// permitir expressões arbitrárias) e transforma cada token em prefixo.
fn build_match_expression(query: &str) -> Option<String> {
    let sanitized: String = query
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect();

    let tokens: Vec<String> = sanitized
        .split_whitespace()
        .map(|token| format!("{token}*"))
        .collect();

    if tokens.is_empty() {
        return None;
    }

    Some(tokens.join(" AND "))
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn available_flag(available: bool) -> i64 {
    if available { 1 } else { 0 }
}

fn as_i64(value: u64) -> Result<i64> {
    i64::try_from(value).with_context(|| format!("valor {value} não cabe em um inteiro de 64 bits"))
}

fn optional_i64(value: Option<u64>) -> Result<Option<i64>> {
    match value {
        Some(value) => Ok(Some(as_i64(value)?)),
        None => Ok(None),
    }
}

fn as_u64(value: i64) -> Result<u64> {
    u64::try_from(value).with_context(|| format!("valor {value} não pode ser negativo"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_expression_uses_prefix_tokens() {
        assert_eq!(
            build_match_expression("  Caetano   Veloso "),
            Some("Caetano* AND Veloso*".to_string())
        );
        assert_eq!(
            build_match_expression("a\"b OR (c)"),
            Some("a* AND b* AND OR* AND c*".to_string())
        );
    }

    #[test]
    fn match_expression_ignores_punctuation_only_queries() {
        assert_eq!(build_match_expression(""), None);
        assert_eq!(build_match_expression("  -- \"\"  "), None);
    }
}

/// Capa persistida (referência ao arquivo no cache).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkRecord {
    pub id: String,
    pub relative_path: String,
    pub mime_type: Option<String>,
    pub checksum: String,
}

/// Busca a capa por id.
pub async fn get_artwork(pool: &SqlitePool, artwork_id: &str) -> Result<Option<ArtworkRecord>> {
    use sqlx::Row;

    let row =
        sqlx::query("SELECT id, relative_path, mime_type, checksum FROM artworks WHERE id = ?1")
            .bind(artwork_id)
            .fetch_optional(pool)
            .await
            .context("falha ao consultar capa")?;

    row.map(|row| {
        Ok(ArtworkRecord {
            id: row.try_get("id")?,
            relative_path: row.try_get("relative_path")?,
            mime_type: row.try_get("mime_type")?,
            checksum: row.try_get("checksum")?,
        })
    })
    .transpose()
}

/// Resolve a faixa e o arquivo que devem ser servidos pelo endpoint de streaming.
///
/// Prioriza arquivos disponíveis e, entre eles, o último indexado.
pub async fn get_stream_target(pool: &SqlitePool, track_id: &str) -> Result<Option<StreamTarget>> {
    use sqlx::Row;

    let row = sqlx::query(
        "SELECT t.id AS track_id, t.title, af.relative_path, af.sha256, af.size_bytes, af.available
         FROM tracks t
         JOIN audio_files af ON af.track_id = t.id
         WHERE t.id = ?1
         ORDER BY af.available DESC, af.scanned_at DESC
         LIMIT 1",
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await
    .context("falha ao resolver o alvo de streaming")?;

    row.map(|row| {
        Ok(StreamTarget {
            track_id: row.try_get("track_id")?,
            title: row.try_get("title")?,
            relative_path: row.try_get("relative_path")?,
            sha256: row.try_get("sha256")?,
            size_bytes: u64::try_from(row.try_get::<i64, _>("size_bytes")?)?,
            available: row.try_get::<i64, _>("available")? == 1,
        })
    })
    .transpose()
}

/// Retorna o `track_id` associado a um caminho relativo (se indexado).
pub async fn find_track_id_by_path(
    pool: &SqlitePool,
    relative_path: &str,
) -> Result<Option<String>> {
    sqlx::query_scalar("SELECT track_id FROM audio_files WHERE relative_path = ?1")
        .bind(relative_path)
        .fetch_optional(pool)
        .await
        .context("falha ao consultar faixa pelo caminho")
}

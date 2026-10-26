//! Persistência de uploads (staging → indexed/duplicate/failed).

use anyhow::{Context, Result};
use sqlx::{Row, SqlitePool};

use super::NOW_SQL;

/// Dados iniciais de um upload em staging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUpload {
    pub id: String,
    pub original_filename: String,
    pub staging_path: String,
}

/// Upload persistido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadRecord {
    pub id: String,
    pub original_filename: String,
    pub staging_path: String,
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub status: String,
    pub track_id: Option<String>,
    pub error: Option<String>,
}

/// Cria a linha do upload já em `staged`.
pub async fn insert_upload(pool: &SqlitePool, upload: &NewUpload) -> Result<()> {
    sqlx::query(
        "INSERT INTO uploads (id, original_filename, staging_path, status)
         VALUES (?1, ?2, ?3, 'staged')",
    )
    .bind(&upload.id)
    .bind(&upload.original_filename)
    .bind(&upload.staging_path)
    .execute(pool)
    .await
    .with_context(|| format!("falha ao criar o upload '{}'", upload.id))?;

    Ok(())
}

/// Atualiza tamanho e sha256 depois que o corpo foi gravado em staging.
pub async fn set_staged_metadata(
    pool: &SqlitePool,
    id: &str,
    size_bytes: u64,
    sha256: &str,
) -> Result<()> {
    let size_bytes = i64::try_from(size_bytes)
        .with_context(|| format!("tamanho {size_bytes} não cabe em um inteiro de 64 bits"))?;

    let result = sqlx::query(
        "UPDATE uploads SET size_bytes = ?2, sha256 = ?3
         WHERE id = ?1 AND status = 'staged'",
    )
    .bind(id)
    .bind(size_bytes)
    .bind(sha256)
    .execute(pool)
    .await
    .with_context(|| format!("falha ao registrar os metadados do upload '{id}'"))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("upload '{id}' não encontrado (ou já finalizado)");
    }

    Ok(())
}

/// Busca um upload pelo id.
pub async fn get_upload(pool: &SqlitePool, id: &str) -> Result<Option<UploadRecord>> {
    let row = sqlx::query(
        "SELECT id, original_filename, staging_path, size_bytes, sha256, status, track_id, error
         FROM uploads
         WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("falha ao consultar o upload '{id}'"))?;

    row.map(|row| {
        let size_bytes = row.try_get::<i64, _>("size_bytes")?;
        let size_bytes = u64::try_from(size_bytes)
            .with_context(|| format!("tamanho negativo no upload '{id}'"))?;

        Ok(UploadRecord {
            id: row.try_get("id")?,
            original_filename: row.try_get("original_filename")?,
            staging_path: row.try_get("staging_path")?,
            size_bytes,
            sha256: row.try_get("sha256")?,
            status: row.try_get("status")?,
            track_id: row.try_get("track_id")?,
            error: row.try_get("error")?,
        })
    })
    .transpose()
}

/// Marca o estado final do upload (`indexed`, `duplicate` ou `failed`).
pub async fn finish_upload(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    track_id: Option<&str>,
    error: Option<&str>,
) -> Result<()> {
    let result = sqlx::query(&format!(
        "UPDATE uploads SET
             status = ?2,
             track_id = ?3,
             error = ?4,
             completed_at = {NOW_SQL}
         WHERE id = ?1"
    ))
    .bind(id)
    .bind(status)
    .bind(track_id)
    .bind(error)
    .execute(pool)
    .await
    .with_context(|| format!("falha ao finalizar o upload '{id}'"))?;

    if result.rows_affected() == 0 {
        anyhow::bail!("upload '{id}' não encontrado ao finalizar");
    }

    Ok(())
}

/// Remove a linha (usado apenas no cancelamento explícito do staging).
pub async fn delete_upload(pool: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM uploads WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .with_context(|| format!("falha ao remover o upload '{id}'"))?;

    Ok(())
}

/// Retorna o `track_id` cujo arquivo já tem este sha256.
pub async fn find_track_id_by_sha(pool: &SqlitePool, sha256: &str) -> Result<Option<String>> {
    sqlx::query_scalar::<_, String>(
        "SELECT track_id FROM audio_files
         WHERE sha256 = ?1
         ORDER BY available DESC, scanned_at DESC
         LIMIT 1",
    )
    .bind(sha256)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("falha ao procurar a faixa com sha256 '{sha256}'"))
}

/// Remove linhas de upload presas em `staged` há mais de `older_than_seconds`.
pub async fn purge_stale_uploads(pool: &SqlitePool, older_than_seconds: i64) -> Result<u64> {
    if older_than_seconds < 0 {
        anyhow::bail!("older_than_seconds não pode ser negativo");
    }

    let modifier = format!("-{older_than_seconds} seconds");
    let result = sqlx::query(
        "DELETE FROM uploads
         WHERE status = 'staged'
           AND created_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1)",
    )
    .bind(modifier)
    .execute(pool)
    .await
    .context("falha ao limpar uploads antigos em staging")?;

    Ok(result.rows_affected())
}

//! Endpoints de upload: staging, status, finalização e cancelamento.

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Serialize;

use super::AppState;
use crate::db::Database;
use crate::error::ApiError;
use crate::library::metadata;
use crate::library::{LibraryScanner, ScanOptions};
use crate::repository::{self, uploads as uploads_repo};
use crate::upload::{UploadService, UploadStatus};

/// Resposta de `POST /api/v1/uploads`.
#[derive(Debug, Serialize)]
pub struct CreateUploadResponse {
    pub upload_id: String,
    pub original_filename: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub status: String,
}

/// Resposta de `GET /api/v1/uploads/{id}`.
#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub upload_id: String,
    pub original_filename: String,
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub status: String,
    pub track_id: Option<String>,
    pub error: Option<String>,
}

/// Resposta de `POST /api/v1/uploads/{id}/complete`.
#[derive(Debug, Serialize)]
pub struct CompleteUploadResponse {
    pub upload_id: String,
    pub status: String,
    pub track_id: Option<String>,
}

/// `POST /api/v1/uploads` — corpo binário bruto + cabeçalho `X-Zuno-Filename`.
pub async fn create_upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body,
) -> Result<(StatusCode, Json<CreateUploadResponse>), ApiError> {
    let service = UploadService::new(&state.config)?;

    let filename = headers
        .get("x-zuno-filename")
        .and_then(|value| value.to_str().ok());

    let staged = service.stage(filename, body).await?;

    let new_upload = uploads_repo::NewUpload {
        id: staged.id.clone(),
        original_filename: staged.original_filename.clone(),
        staging_path: service
            .finalized_path(&staged.id)
            .to_string_lossy()
            .to_string(),
    };

    uploads_repo::insert_upload(state.pool(), &new_upload).await?;
    uploads_repo::set_staged_metadata(state.pool(), &staged.id, staged.size_bytes, &staged.sha256)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateUploadResponse {
            upload_id: staged.id,
            original_filename: staged.original_filename,
            size_bytes: staged.size_bytes,
            sha256: staged.sha256,
            status: UploadStatus::Staged.as_str().to_string(),
        }),
    ))
}

/// `GET /api/v1/uploads/{id}`
pub async fn get_upload(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
) -> Result<Json<UploadResponse>, ApiError> {
    let record = uploads_repo::get_upload(state.pool(), &upload_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(UploadResponse {
        upload_id: record.id,
        original_filename: record.original_filename,
        size_bytes: record.size_bytes,
        sha256: record.sha256,
        status: record.status,
        track_id: record.track_id,
        error: record.error,
    }))
}

/// `POST /api/v1/uploads/{id}/complete`
pub async fn complete_upload(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
) -> Result<(StatusCode, Json<CompleteUploadResponse>), ApiError> {
    let service = UploadService::new(&state.config)?;
    let pool = state.pool();

    let record = uploads_repo::get_upload(pool, &upload_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    if record.status != UploadStatus::Staged.as_str() {
        return Err(ApiError::NotPending);
    }

    let Some(sha256) = record.sha256.clone() else {
        return Err(ApiError::Internal(anyhow::anyhow!(
            "upload em staging sem sha256"
        )));
    };

    // Duplicata exata: a biblioteca fica intocada; só o staging é descartado.
    if let Some(track_id) = uploads_repo::find_track_id_by_sha(pool, &sha256).await? {
        uploads_repo::finish_upload(
            pool,
            &record.id,
            UploadStatus::Duplicate.as_str(),
            Some(&track_id),
            None,
        )
        .await?;

        if let Err(error) = service.remove_staging(&record).await {
            tracing::warn!(error = %error, "falha ao remover staging de upload duplicado");
        }

        return Ok((
            StatusCode::CONFLICT,
            Json(CompleteUploadResponse {
                upload_id: record.id,
                status: UploadStatus::Duplicate.as_str().to_string(),
                track_id: Some(track_id),
            }),
        ));
    }

    let staged_path = service.staged_path(&record)?;
    let extraction_path = staged_path.clone();
    let extracted = tokio::task::spawn_blocking(move || metadata::extract(&extraction_path))
        .await
        .map_err(|error| ApiError::Internal(anyhow::anyhow!("tarefa de leitura falhou: {error}")))?
        .map_err(|error| {
            ApiError::Internal(anyhow::anyhow!("falha ao reler metadados: {error}"))
        })?;

    let relative_path = match service
        .place_in_library(
            &state.config,
            &staged_path,
            &extracted,
            &sha256,
            &record.original_filename,
        )
        .await
    {
        Ok(path) => path,
        Err(error) => {
            mark_failed(pool, &record.id, &format!("{error}")).await;
            return Err(error.into());
        }
    };

    // Daqui em diante o arquivo já está na biblioteca: nunca o movemos de volta.
    match index_relative(&state, &relative_path).await {
        Ok(track_id) => {
            uploads_repo::finish_upload(
                pool,
                &record.id,
                UploadStatus::Indexed.as_str(),
                Some(&track_id),
                None,
            )
            .await?;

            Ok((
                StatusCode::OK,
                Json(CompleteUploadResponse {
                    upload_id: record.id,
                    status: UploadStatus::Indexed.as_str().to_string(),
                    track_id: Some(track_id),
                }),
            ))
        }
        Err(error) => {
            mark_failed(pool, &record.id, &format!("{error:#}")).await;
            Err(ApiError::Internal(error))
        }
    }
}

/// `DELETE /api/v1/uploads/{id}` — descarta apenas o arquivo em staging.
pub async fn delete_upload(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let service = UploadService::new(&state.config)?;
    let record = uploads_repo::get_upload(state.pool(), &upload_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Só o staging sai do disco; a biblioteca nunca é tocada.
    service.remove_staging(&record).await?;
    uploads_repo::delete_upload(state.pool(), &record.id).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn mark_failed(pool: &sqlx::SqlitePool, upload_id: &str, error: &str) {
    if let Err(update_error) = uploads_repo::finish_upload(
        pool,
        upload_id,
        UploadStatus::Failed.as_str(),
        None,
        Some(error),
    )
    .await
    {
        tracing::error!(
            error = %update_error,
            "falha ao marcar upload como failed"
        );
    }
}

async fn index_relative(state: &AppState, relative_path: &str) -> anyhow::Result<String> {
    let scanner = LibraryScanner::new(&state.config, ScanOptions::from_config(&state.config))?;
    let database = Database::from_pool(state.pool().clone());

    let issues = scanner
        .index_relative_path(&database, relative_path)
        .await?;

    if let Some(track_id) = repository::find_track_id_by_path(state.pool(), relative_path).await? {
        return Ok(track_id);
    }

    let details: Vec<String> = issues
        .iter()
        .map(|issue| match issue.detail.as_deref() {
            Some(detail) => format!("{} ({detail})", issue.code),
            None => issue.code.to_string(),
        })
        .collect();

    anyhow::bail!(
        "arquivo movido para a biblioteca mas não foi indexado: {}",
        details.join("; ")
    )
}

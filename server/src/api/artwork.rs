//! Capas: serve o arquivo original ou uma variante redimensionada em cache.
//!
//! O cache é imutável por construção: o `id` da capa vem do checksum do conteúdo,
//! então uma capa nova é um `id` novo. A escrita é atômica (`.tmp` + rename).
//! Nada é servido de fora de `ZUNO_ARTWORK_DIR`.

use std::path::PathBuf;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use tokio::io::AsyncReadExt;

use super::{AppState, resolve_within};
use crate::error::ApiError;
use crate::repository;

const DEFAULT_DERIVED_SIZE: u32 = 320;
const MIN_DERIVED_SIZE: u32 = 16;
const MAX_DERIVED_SIZE: u32 = 2048;
const JPEG_QUALITY: u8 = 82;

/// Parâmetros aceitos por `GET /api/v1/artworks/{id}`.
#[derive(Debug, Deserialize)]
pub struct ArtworkParams {
    /// `full` (padrão) devolve os bytes originais; `thumb`/`card` ou um número
    /// devolvem uma variante quadrada redimensionada em cache.
    pub size: Option<String>,
}

/// `GET /api/v1/artworks/{id}`
pub async fn get_artwork(
    State(state): State<AppState>,
    Path(artwork_id): Path<String>,
    Query(params): Query<ArtworkParams>,
) -> Result<Response, ApiError> {
    let record = repository::get_artwork(state.pool(), &artwork_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    if let Some(size) = parse_size(params.size.as_deref())? {
        return serve_derived(&state, &record, size).await;
    }

    serve_original(state, record).await
}

async fn serve_original(
    state: AppState,
    record: repository::ArtworkRecord,
) -> Result<Response, ApiError> {
    let root = state.config.artwork_dir.clone();
    let relative = record.relative_path.clone();
    let path = tokio::task::spawn_blocking(move || resolve_within(&root, &relative))
        .await
        .map_err(|error| {
            ApiError::Internal(anyhow::anyhow!("tarefa de resolução falhou: {error}"))
        })??;

    let mut file = tokio::fs::File::open(&path).await.map_err(|error| {
        tracing::warn!(error = %error, "capa indexada não pôde ser aberta");
        ApiError::NotFound
    })?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .await
        .map_err(|error| ApiError::Internal(anyhow::anyhow!("falha ao ler capa: {error}")))?;

    let mime = record
        .mime_type
        .clone()
        .or_else(|| infer::get(&bytes).map(|kind| kind.mime_type().to_string()))
        .unwrap_or_else(|| "application/octet-stream".to_string());

    Ok(build_response(StatusCode::OK, &mime, bytes))
}

async fn serve_derived(
    state: &AppState,
    record: &repository::ArtworkRecord,
    size: u32,
) -> Result<Response, ApiError> {
    let root = state.config.artwork_dir.clone();
    let relative = record.relative_path.clone();
    let source = tokio::task::spawn_blocking(move || resolve_within(&root, &relative))
        .await
        .map_err(|error| {
            ApiError::Internal(anyhow::anyhow!("tarefa de resolução falhou: {error}"))
        })??;

    let derived_root = state.config.artwork_dir.join("derived");
    let derived_path: PathBuf = derived_root.join(format!("{}-{size}.jpg", record.id));

    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, ApiError> {
        if derived_path.exists() {
            return std::fs::read(&derived_path).map_err(|error| {
                ApiError::Internal(anyhow::anyhow!("falha ao ler capa: {error}"))
            });
        }

        let image = image::open(&source).map_err(|error| {
            tracing::warn!(error = %error, "capa não pôde ser decodificada");
            ApiError::Internal(anyhow::anyhow!("capa inválida: {error}"))
        })?;
        let thumbnail = image.thumbnail(size, size);

        std::fs::create_dir_all(&derived_root).map_err(|error| {
            ApiError::Internal(anyhow::anyhow!("falha ao criar cache de capas: {error}"))
        })?;

        let mut buffer = Vec::new();
        {
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, JPEG_QUALITY);
            encoder.encode_image(&thumbnail).map_err(|error| {
                ApiError::Internal(anyhow::anyhow!("falha ao codificar capa: {error}"))
            })?;
        }

        let temporary = derived_path.with_extension("jpg.tmp");
        std::fs::write(&temporary, &buffer).map_err(|error| {
            ApiError::Internal(anyhow::anyhow!("falha ao gravar capa temporária: {error}"))
        })?;
        std::fs::rename(&temporary, &derived_path).map_err(|error| {
            ApiError::Internal(anyhow::anyhow!("falha ao publicar capa: {error}"))
        })?;

        Ok(buffer)
    })
    .await
    .map_err(|error| {
        ApiError::Internal(anyhow::anyhow!(
            "tarefa de redimensionamento falhou: {error}"
        ))
    })??;

    Ok(build_response(StatusCode::OK, "image/jpeg", bytes))
}

fn build_response(status: StatusCode, mime: &str, bytes: Vec<u8>) -> Response {
    let length = bytes.len();
    let mut response = (status, Body::from(bytes)).into_response();
    let headers = response.headers_mut();

    if let Ok(value) = HeaderValue::from_str(mime) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=86400"),
    );
    if let Ok(value) = HeaderValue::from_str(&length.to_string()) {
        headers.insert(header::CONTENT_LENGTH, value);
    }

    response
}

/// Converte o parâmetro `size` em um lado máximo em pixels.
fn parse_size(value: Option<&str>) -> Result<Option<u32>, ApiError> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    match raw.to_ascii_lowercase().as_str() {
        "full" => Ok(None),
        "thumb" => Ok(Some(DEFAULT_DERIVED_SIZE)),
        "card" => Ok(Some(640)),
        other => {
            let size: u32 = other
                .parse()
                .map_err(|_| ApiError::InvalidQuery(format!("size={other}")))?;
            Ok(Some(size.clamp(MIN_DERIVED_SIZE, MAX_DERIVED_SIZE)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_parameter_is_interpreted_safely() {
        assert_eq!(parse_size(None).unwrap(), None);
        assert_eq!(parse_size(Some("full")).unwrap(), None);
        assert_eq!(parse_size(Some("thumb")).unwrap(), Some(320));
        assert_eq!(parse_size(Some("card")).unwrap(), Some(640));
        assert_eq!(parse_size(Some("100")).unwrap(), Some(100));
        assert_eq!(parse_size(Some("5")).unwrap(), Some(16));
        assert_eq!(parse_size(Some("99999")).unwrap(), Some(2048));
        assert!(parse_size(Some("grande")).is_err());
    }
}

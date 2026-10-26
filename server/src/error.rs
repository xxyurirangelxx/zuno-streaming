//! Erros da API com representação JSON estável.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

use crate::upload::UploadError;

/// Corpo padrão de erro da API: `{ "code": "snake_case", "message": "texto" }`.
#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

/// Erro de requisição HTTP convertido automaticamente em resposta JSON.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Recurso solicitado não existe.
    #[error("recurso não encontrado")]
    NotFound,
    /// Parâmetro de paginação inválido.
    #[error("cursor de paginação inválido")]
    InvalidCursor,
    /// Parâmetro de consulta inválido.
    #[error("parâmetro de consulta inválido: {0}")]
    InvalidQuery(String),
    /// Intervalo solicitado fora do tamanho do recurso.
    #[error("intervalo solicitado fora do recurso")]
    RangeNotSatisfiable { resource_size: u64 },
    /// A operação excedeu o tempo limite permitido.
    #[error("tempo limite excedido")]
    Timeout,
    /// Dependência temporariamente indisponível.
    #[error("serviço temporariamente indisponível")]
    ServiceUnavailable,
    /// Upload maior que o limite configurado.
    #[error("arquivo excede o limite de tamanho")]
    TooLarge,
    /// Upload de áudio que não é Opus (ADR-0007).
    #[error("formato não suportado: {0}")]
    UnsupportedFormat(String),
    /// Arquivo Opus ilegível/corrompido.
    #[error("arquivo Opus inválido: {0}")]
    InvalidAudio(String),
    /// Cabeçalho com o nome do arquivo ausente/inválido.
    #[error("nome de arquivo ausente ou inválido")]
    InvalidFilename,
    /// Upload já finalizado (não está mais em staging).
    #[error("upload não está pendente")]
    NotPending,
    /// Conteúdo já existe na biblioteca.
    #[error("arquivo já existe na biblioteca")]
    Duplicate { track_id: String },
    /// Falha inesperada, registrada no log e ocultada do cliente.
    #[error("erro interno do servidor")]
    Internal(#[source] anyhow::Error),
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self::Internal(error)
    }
}

impl From<UploadError> for ApiError {
    fn from(error: UploadError) -> Self {
        match error {
            UploadError::TooLarge { .. } => Self::TooLarge,
            UploadError::UnsupportedFormat(detail) => Self::UnsupportedFormat(detail),
            UploadError::InvalidAudio(detail) => Self::InvalidAudio(detail),
            UploadError::InvalidFilename => Self::InvalidFilename,
            UploadError::NotFound => Self::NotFound,
            UploadError::NotPending => Self::NotPending,
            UploadError::Duplicate { track_id } => Self::Duplicate { track_id },
            UploadError::Io(error) => Self::Internal(anyhow::Error::new(error)),
            UploadError::Internal(error) => Self::Internal(error),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let range_size = match &self {
            Self::RangeNotSatisfiable { resource_size } => Some(*resource_size),
            _ => None,
        };

        let (status, code, message) = match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "recurso não encontrado".to_string(),
            ),
            Self::InvalidCursor => (
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                "cursor de paginação inválido".to_string(),
            ),
            Self::InvalidQuery(detail) => (
                StatusCode::BAD_REQUEST,
                "invalid_query",
                format!("parâmetro de consulta inválido: {detail}"),
            ),
            Self::RangeNotSatisfiable { resource_size } => (
                StatusCode::RANGE_NOT_SATISFIABLE,
                "range_not_satisfiable",
                format!(
                    "intervalo solicitado fora do recurso de {resource_size} bytes; \
                     use Range: bytes=0-"
                ),
            ),
            Self::Timeout => (
                StatusCode::REQUEST_TIMEOUT,
                "timeout",
                "tempo limite excedido".to_string(),
            ),
            Self::ServiceUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "serviço temporariamente indisponível".to_string(),
            ),
            Self::TooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                "arquivo excede o limite de tamanho".to_string(),
            ),
            Self::UnsupportedFormat(detail) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_format",
                format!("formato não suportado: {detail}"),
            ),
            Self::InvalidAudio(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_audio",
                "arquivo Opus inválido".to_string(),
            ),
            Self::InvalidFilename => (
                StatusCode::BAD_REQUEST,
                "invalid_filename",
                "nome de arquivo ausente ou inválido".to_string(),
            ),
            Self::NotPending => (
                StatusCode::CONFLICT,
                "not_pending",
                "upload não está pendente".to_string(),
            ),
            Self::Duplicate { .. } => (
                StatusCode::CONFLICT,
                "duplicate",
                "arquivo já existe na biblioteca".to_string(),
            ),
            Self::Internal(error) => {
                tracing::error!(error = %error, "erro interno ao processar requisição");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "erro interno do servidor".to_string(),
                )
            }
        };

        let mut response = (status, Json(ErrorBody { code, message })).into_response();

        if let Some(resource_size) = range_size
            && let Ok(value) = HeaderValue::from_str(&format!("bytes */{resource_size}"))
        {
            response.headers_mut().insert(header::CONTENT_RANGE, value);
        }

        response
    }
}

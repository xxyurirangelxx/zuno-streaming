//! Erros da API com representação JSON estável.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

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
    /// A operação excedeu o tempo limite permitido.
    #[error("tempo limite excedido")]
    Timeout,
    /// Dependência temporariamente indisponível.
    #[error("serviço temporariamente indisponível")]
    ServiceUnavailable,
    /// Falha inesperada, registrada no log e ocultada do cliente.
    #[error("erro interno do servidor")]
    Internal(#[source] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "recurso não encontrado".to_string(),
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
            Self::Internal(error) => {
                tracing::error!(error = %error, "erro interno ao processar requisição");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "erro interno do servidor".to_string(),
                )
            }
        };

        (status, Json(ErrorBody { code, message })).into_response()
    }
}

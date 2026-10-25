//! Endpoint de saúde do servidor.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;

use super::AppState;

/// Estado do subsistema de banco de dados.
#[derive(Debug, Serialize)]
pub struct DatabaseHealth {
    /// `ok` quando a consulta trivial responde; `error` caso contrário.
    pub status: &'static str,
}

/// Resposta do contrato `GET /health`.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// `ok` ou `degraded`.
    pub status: &'static str,
    /// Nome canônico do serviço.
    pub service: &'static str,
    /// Versão do crate do servidor.
    pub version: &'static str,
    /// Segundos desde o início do processo.
    pub uptime_seconds: u64,
    /// Saúde do banco de dados.
    pub database: DatabaseHealth,
}

/// Responde 200 com `ok` ou 503 com `degraded` conforme o banco.
pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let database_healthy = sqlx::query("SELECT 1").execute(state.pool()).await.is_ok();

    if !database_healthy {
        tracing::warn!("healthcheck: banco de dados indisponível");
    }

    let (status_code, status, database_status) = if database_healthy {
        (StatusCode::OK, "ok", "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "degraded", "error")
    };

    let body = HealthResponse {
        status,
        service: "zuno-server",
        version: state.version,
        uptime_seconds: state.started_at.elapsed().as_secs(),
        database: DatabaseHealth {
            status: database_status,
        },
    };

    (status_code, Json(body))
}

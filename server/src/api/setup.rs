//! Setup inicial: reivindicação única do primeiro administrador.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use super::AppState;
use super::auth::{create_user_with_session, validate_username};
use crate::auth::{Role, SESSION_TTL, hash_password, hash_token, session_cookie};
use crate::error::ApiError;
use crate::repository::auth as auth_repo;

/// `GET /api/v1/setup/status`
#[derive(Debug, Serialize)]
pub struct SetupStatus {
    /// `true` enquanto a instância não tiver usuários ou houver código pendente.
    pub required: bool,
}

/// Corpo de `POST /api/v1/setup/claim`.
#[derive(Debug, Deserialize)]
pub struct SetupClaimRequest {
    pub code: String,
    pub username: String,
    pub password: String,
}

/// `GET /api/v1/setup/status`
pub async fn status(State(state): State<AppState>) -> Result<Json<SetupStatus>, ApiError> {
    let users = auth_repo::count_users(state.pool())
        .await
        .map_err(ApiError::Internal)?;
    let pending = auth_repo::has_valid_setup_token(state.pool())
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(SetupStatus {
        required: users == 0 || pending,
    }))
}

/// `POST /api/v1/setup/claim` — consome o código e cria o administrador.
pub async fn claim(
    State(state): State<AppState>,
    Json(payload): Json<SetupClaimRequest>,
) -> Result<Response, ApiError> {
    let users = auth_repo::count_users(state.pool())
        .await
        .map_err(ApiError::Internal)?;

    if users > 0 {
        return Err(ApiError::Conflict("setup_completed"));
    }

    let code = payload.code.trim().to_ascii_uppercase();
    if code.is_empty() {
        return Err(ApiError::InvalidCredentials);
    }

    let consumed = auth_repo::consume_setup_token(state.pool(), &hash_token(&code))
        .await
        .map_err(ApiError::Internal)?;

    if !consumed {
        return Err(ApiError::InvalidCredentials);
    }

    // Validações antes de criar (o código já foi consumido: uso único).
    validate_username(&payload.username)?;
    hash_password(&payload.password).map_err(|_| {
        ApiError::InvalidQuery("senha deve ter pelo menos 8 caracteres".to_string())
    })?;

    let (user, token) =
        create_user_with_session(&state, &payload.username, &payload.password, Role::Admin).await?;

    auth_repo::record_audit(
        state.pool(),
        Some(&user.id),
        "setup.claim",
        Some(&user.id),
        Some("primeiro administrador criado"),
    )
    .await
    .map_err(ApiError::Internal)?;

    let cookie = session_cookie(&token, state.config.cookie_secure, SESSION_TTL);
    let mut response = (
        StatusCode::CREATED,
        Json(serde_json::json!({ "user": user })),
    )
        .into_response();

    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }

    Ok(response)
}

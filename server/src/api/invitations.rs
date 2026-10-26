//! Convites: criação/revogação por administrador e redenção pública.

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use super::auth::{create_user_with_session, validate_username};
use crate::auth::middleware::require_admin;
use crate::auth::{
    CurrentUser, Role, SESSION_TTL, generate_token, hash_password, hash_token, session_cookie,
};
use crate::error::ApiError;
use crate::repository::auth as auth_repo;

/// Corpo de `POST /api/v1/invitations`.
#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequest {
    /// `user` (padrão) ou `moderator`. `admin` só é criado no setup.
    pub role: Option<String>,
    pub expires_in_days: Option<i64>,
}

/// Resposta da criação (token aparece **uma única vez**).
#[derive(Debug, Serialize)]
pub struct CreatedInvitation {
    pub id: String,
    pub token: String,
    pub role: Role,
    pub expires_in_days: i64,
}

/// Item de `GET /api/v1/invitations` (sem token).
#[derive(Debug, Serialize)]
pub struct InvitationItem {
    pub id: String,
    pub role: Role,
    pub expires_at: String,
    pub redeemed_at: Option<String>,
    pub revoked_at: Option<String>,
}

/// Corpo de `POST /api/v1/invitations/redeem`.
#[derive(Debug, Deserialize)]
pub struct RedeemInvitationRequest {
    pub token: String,
    pub username: String,
    pub password: String,
}

/// `POST /api/v1/invitations` (admin)
pub async fn create(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Json(payload): Json<CreateInvitationRequest>,
) -> Result<Json<CreatedInvitation>, ApiError> {
    require_admin(&current)?;

    let role = match payload.role.as_deref().unwrap_or("user") {
        "user" => Role::User,
        "moderator" => Role::Moderator,
        "admin" => {
            return Err(ApiError::InvalidQuery(
                "convite não cria administrador; use outro admin ou o setup".to_string(),
            ));
        }
        other => {
            return Err(ApiError::InvalidQuery(format!("role inválido: {other}")));
        }
    };

    let days = payload.expires_in_days.unwrap_or(7).clamp(1, 30);
    let token = generate_token();
    let id = Uuid::now_v7().to_string();

    auth_repo::insert_invitation(
        state.pool(),
        &id,
        &hash_token(&token),
        role,
        Some(&current.id),
        &format!("+{days} days"),
    )
    .await
    .map_err(ApiError::Internal)?;

    auth_repo::record_audit(
        state.pool(),
        Some(&current.id),
        "invitation.create",
        Some(&id),
        Some(role.as_str()),
    )
    .await
    .map_err(ApiError::Internal)?;

    Ok(Json(CreatedInvitation {
        id,
        token,
        role,
        expires_in_days: days,
    }))
}

/// `GET /api/v1/invitations` (admin)
pub async fn list(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<Vec<InvitationItem>>, ApiError> {
    require_admin(&current)?;

    let records = auth_repo::list_invitations(state.pool())
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(
        records
            .into_iter()
            .map(|record| InvitationItem {
                id: record.id,
                role: record.role,
                expires_at: record.expires_at,
                redeemed_at: record.redeemed_at,
                revoked_at: record.revoked_at,
            })
            .collect(),
    ))
}

/// `DELETE /api/v1/invitations/{id}` (admin)
pub async fn revoke(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(invitation_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&current)?;

    let revoked = auth_repo::revoke_invitation(state.pool(), &invitation_id)
        .await
        .map_err(ApiError::Internal)?;

    if !revoked {
        return Err(ApiError::NotFound);
    }

    auth_repo::record_audit(
        state.pool(),
        Some(&current.id),
        "invitation.revoke",
        Some(&invitation_id),
        None,
    )
    .await
    .map_err(ApiError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/invitations/redeem` (público, com token)
pub async fn redeem(
    State(state): State<AppState>,
    Json(payload): Json<RedeemInvitationRequest>,
) -> Result<Response, ApiError> {
    let token = payload.token.trim();
    if token.is_empty() {
        return Err(ApiError::InvalidCredentials);
    }

    let invitation = auth_repo::find_valid_invitation(state.pool(), &hash_token(token))
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::InvalidCredentials)?;

    validate_username(&payload.username)?;
    hash_password(&payload.password).map_err(|_| {
        ApiError::InvalidQuery("senha deve ter pelo menos 8 caracteres".to_string())
    })?;

    let consumed = auth_repo::consume_invitation(state.pool(), &invitation.id)
        .await
        .map_err(ApiError::Internal)?;

    if !consumed {
        return Err(ApiError::InvalidCredentials);
    }

    let (user, session_token) = create_user_with_session(
        &state,
        &payload.username,
        &payload.password,
        invitation.role,
    )
    .await?;

    auth_repo::record_audit(
        state.pool(),
        Some(&user.id),
        "invitation.redeem",
        Some(&invitation.id),
        Some(user.role.as_str()),
    )
    .await
    .map_err(ApiError::Internal)?;

    let cookie = session_cookie(&session_token, state.config.cookie_secure, SESSION_TTL);
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

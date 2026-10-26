//! Endpoints de sessão: login, logout, refresh e identidade atual.

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use crate::auth::{
    AuthError, CurrentUser, LoginRateLimiter, Role, SESSION_TTL, clear_session_cookie,
    generate_token, hash_password, hash_token, session_cookie, verify_password,
};
use crate::domain::normalize_key;
use crate::error::ApiError;
use crate::repository::auth as auth_repo;

/// Corpo de `POST /api/v1/auth/login` e `POST /api/v1/setup/claim`.
#[derive(Debug, Deserialize)]
pub struct CredentialsRequest {
    pub username: String,
    pub password: String,
}

/// Usuário exposto na API (sem hash de senha).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub role: Role,
}

impl From<&CurrentUser> for UserResponse {
    fn from(user: &CurrentUser) -> Self {
        Self {
            id: user.id.clone(),
            username: user.username.clone(),
            role: user.role,
        }
    }
}

/// `POST /api/v1/auth/login`
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<CredentialsRequest>,
) -> Result<Response, ApiError> {
    let username = normalize_key(&payload.username);

    if username.is_empty() || payload.password.is_empty() {
        return Err(ApiError::InvalidCredentials);
    }

    state
        .login_limiter
        .check_and_record(&LoginRateLimiter::key(&username, None))
        .map_err(|error| match error {
            AuthError::TooManyAttempts => ApiError::TooManyAttempts,
            _ => ApiError::InvalidCredentials,
        })?;

    let user = auth_repo::find_user_by_username(state.pool(), &username)
        .await
        .map_err(ApiError::Internal)?;

    let Some(user) = user else {
        // Mesma resposta para usuário inexistente e senha errada.
        return Err(ApiError::InvalidCredentials);
    };

    if user.disabled {
        return Err(ApiError::Forbidden);
    }

    if !verify_password(&payload.password, &user.password_hash) {
        return Err(ApiError::InvalidCredentials);
    }

    state
        .login_limiter
        .reset(&LoginRateLimiter::key(&username, None));

    let session = create_session(&state, &user.id).await?;
    Ok(session_response(
        &state,
        UserResponse {
            id: user.id,
            username: user.username,
            role: user.role,
        },
        &session,
        StatusCode::OK,
    ))
}

/// `POST /api/v1/auth/refresh` — rotaciona o token da sessão atual.
pub async fn refresh(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Response, ApiError> {
    auth_repo::revoke_session(state.pool(), &current.session_id)
        .await
        .map_err(ApiError::Internal)?;

    let session = create_session(&state, &current.id).await?;
    Ok(session_response(
        &state,
        UserResponse::from(&current),
        &session,
        StatusCode::OK,
    ))
}

/// `POST /api/v1/auth/logout`
pub async fn logout(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Response, ApiError> {
    auth_repo::revoke_session(state.pool(), &current.session_id)
        .await
        .map_err(ApiError::Internal)?;

    let cookie = clear_session_cookie(state.config.cookie_secure);
    let mut response = StatusCode::NO_CONTENT.into_response();
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    Ok(response)
}

/// `GET /api/v1/me`
pub async fn me(Extension(current): Extension<CurrentUser>) -> Json<UserResponse> {
    Json(UserResponse::from(&current))
}

/// Cria sessão persistida e devolve o token em claro (só para o cookie).
async fn create_session(state: &AppState, user_id: &str) -> Result<String, ApiError> {
    let token = generate_token();
    auth_repo::create_session(
        state.pool(),
        &Uuid::now_v7().to_string(),
        user_id,
        &hash_token(&token),
        "+30 days",
    )
    .await
    .map_err(ApiError::Internal)?;

    Ok(token)
}

fn session_response(
    state: &AppState,
    user: UserResponse,
    token: &str,
    status: StatusCode,
) -> Response {
    let cookie = session_cookie(token, state.config.cookie_secure, SESSION_TTL);
    let mut response = (status, Json(serde_json::json!({ "user": user }))).into_response();

    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }

    response
}

/// Cria usuário + sessão (usado pelo setup e pela redenção de convites).
pub(crate) async fn create_user_with_session(
    state: &AppState,
    username: &str,
    password: &str,
    role: Role,
) -> Result<(UserResponse, String), ApiError> {
    let username = validate_username(username)?;
    let password_hash = hash_password(password).map_err(|_| {
        ApiError::InvalidQuery("senha deve ter pelo menos 8 caracteres".to_string())
    })?;

    let id = Uuid::now_v7().to_string();
    auth_repo::insert_user(
        state.pool(),
        &id,
        &username,
        &normalize_key(&username),
        &password_hash,
        role,
    )
    .await
    .map_err(|error| {
        if error.to_string().contains("UNIQUE") {
            ApiError::Conflict("username_taken")
        } else {
            ApiError::Internal(error)
        }
    })?;

    let token = create_session(state, &id).await?;

    Ok((UserResponse { id, username, role }, token))
}

/// Valida e normaliza o nome de usuário (3–32, `[a-zA-Z0-9._-]`).
pub(crate) fn validate_username(raw: &str) -> Result<String, ApiError> {
    let username = raw.trim();

    if !(3..=32).contains(&username.chars().count()) {
        return Err(ApiError::InvalidQuery(
            "nome de usuário deve ter entre 3 e 32 caracteres".to_string(),
        ));
    }

    if !username
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-'))
    {
        return Err(ApiError::InvalidQuery(
            "nome de usuário aceita apenas letras, números, ponto, hífen e sublinhado".to_string(),
        ));
    }

    Ok(username.to_string())
}

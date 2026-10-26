//! Middlewares de sessão e de mesma origem (CSRF).

use axum::extract::{Request, State};
use axum::http::{Method, header};
use axum::middleware::Next;
use axum::response::Response;

use super::{CurrentUser, hash_token, is_same_origin, session_token_from_cookie};
use crate::api::AppState;
use crate::error::ApiError;
use crate::repository::auth as auth_repo;

/// Exige sessão válida e injeta `CurrentUser` nas extensões da requisição.
pub async fn require_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(session_token_from_cookie)
        .ok_or(ApiError::Unauthorized)?;

    let session = auth_repo::find_valid_session(state.pool(), &hash_token(token))
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::Unauthorized)?;

    if session.disabled {
        return Err(ApiError::Forbidden);
    }

    request.extensions_mut().insert(CurrentUser {
        id: session.user_id,
        username: session.username,
        role: session.role,
        session_id: session.session_id,
    });

    Ok(next.run(request).await)
}

/// Rejeita mutações vindas de outra origem (defesa adicional a CSRF).
pub async fn same_origin_guard(request: Request, next: Next) -> Result<Response, ApiError> {
    let is_mutation = matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );

    if is_mutation {
        let headers = request.headers();
        let origin = headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok());
        let host = headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok());
        let fetch_site = headers
            .get("sec-fetch-site")
            .and_then(|value| value.to_str().ok());

        if !is_same_origin(origin, host, fetch_site) {
            tracing::warn!(origin = ?origin, host = ?host, "mutação recusada: origem diferente");
            return Err(ApiError::Forbidden);
        }
    }

    Ok(next.run(request).await)
}

/// Garante papel de administrador (usado dentro dos handlers administrativos).
pub fn require_admin(user: &CurrentUser) -> Result<(), ApiError> {
    if user.role.is_admin() {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

//! Preferências e privacidade do usuário autenticado (M2e).
//!
//! O dono dos dados é sempre o `CurrentUser`: nenhum endpoint aceita `user_id`
//! no corpo ou na query.

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, State};
use serde::{Deserialize, Deserializer};

use super::AppState;
use crate::auth::CurrentUser;
use crate::domain::dto::{Preferences, Privacy};
use crate::error::ApiError;
use crate::repository::preferences;

/// Comprimento mínimo do código de idioma (`en`).
const MIN_LANGUAGE_CHARS: usize = 2;
/// Comprimento máximo do código de idioma (`pt-BR`).
const MAX_LANGUAGE_CHARS: usize = 10;
/// Retenção mínima do histórico, em dias.
const MIN_RETENTION_DAYS: i64 = 1;
/// Retenção máxima do histórico, em dias.
const MAX_RETENTION_DAYS: i64 = 3_650;

/// Corpo de `PUT /api/v1/me/preferences`.
#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesRequest {
    pub theme: Option<String>,
    pub language: Option<String>,
}

/// Corpo de `PUT /api/v1/me/privacy`.
#[derive(Debug, Deserialize)]
pub struct UpdatePrivacyRequest {
    pub allow_cloud_history: Option<bool>,
    /// `None` = campo omitido; `Some(None)` = limpar a retenção (histórico eterno).
    #[serde(default, deserialize_with = "deserialize_some")]
    pub history_retention_days: Option<Option<i64>>,
}

/// `GET /api/v1/me/preferences`
pub async fn get_preferences(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<Preferences>, ApiError> {
    let record = preferences::ensure_preferences(state.pool(), &current.id)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(Preferences {
        theme: record.theme,
        language: record.language,
    }))
}

/// `PUT /api/v1/me/preferences`
pub async fn update_preferences(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    payload: Result<Json<UpdatePreferencesRequest>, JsonRejection>,
) -> Result<Json<Preferences>, ApiError> {
    let Json(payload) = payload.map_err(|_| invalid_body())?;

    let theme = payload.theme.as_deref().map(validate_theme).transpose()?;
    let language = payload
        .language
        .as_deref()
        .map(validate_language)
        .transpose()?;

    let record =
        preferences::update_preferences(state.pool(), &current.id, theme, language.as_deref())
            .await
            .map_err(ApiError::Internal)?;

    Ok(Json(Preferences {
        theme: record.theme,
        language: record.language,
    }))
}

/// `GET /api/v1/me/privacy`
pub async fn get_privacy(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<Privacy>, ApiError> {
    let record = preferences::ensure_privacy(state.pool(), &current.id)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(Privacy {
        allow_cloud_history: record.allow_cloud_history,
        history_retention_days: record.history_retention_days,
    }))
}

/// `PUT /api/v1/me/privacy`
pub async fn update_privacy(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    payload: Result<Json<UpdatePrivacyRequest>, JsonRejection>,
) -> Result<Json<Privacy>, ApiError> {
    let Json(payload) = payload.map_err(|_| invalid_body())?;

    if let Some(Some(days)) = payload.history_retention_days
        && !(MIN_RETENTION_DAYS..=MAX_RETENTION_DAYS).contains(&days)
    {
        return Err(ApiError::InvalidQuery(format!(
            "history_retention_days deve estar entre {MIN_RETENTION_DAYS} e {MAX_RETENTION_DAYS} (ou null)"
        )));
    }

    let record = preferences::update_privacy(
        state.pool(),
        &current.id,
        payload.allow_cloud_history,
        payload.history_retention_days,
    )
    .await
    .map_err(ApiError::Internal)?;

    Ok(Json(Privacy {
        allow_cloud_history: record.allow_cloud_history,
        history_retention_days: record.history_retention_days,
    }))
}

/// Mantém `Some` quando o campo veio como `null`; o `default` cobre a omissão.
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn validate_theme(raw: &str) -> Result<&'static str, ApiError> {
    match raw.trim() {
        "light" => Ok("light"),
        "dark" => Ok("dark"),
        "system" => Ok("system"),
        other => Err(ApiError::InvalidQuery(format!("theme inválido: '{other}'"))),
    }
}

fn validate_language(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    let length = trimmed.chars().count();

    if !(MIN_LANGUAGE_CHARS..=MAX_LANGUAGE_CHARS).contains(&length) {
        return Err(ApiError::InvalidQuery(format!(
            "language deve ter entre {MIN_LANGUAGE_CHARS} e {MAX_LANGUAGE_CHARS} caracteres"
        )));
    }

    if !trimmed
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(ApiError::InvalidQuery(format!(
            "language inválido: '{trimmed}'"
        )));
    }

    Ok(trimmed.to_string())
}

fn invalid_body() -> ApiError {
    ApiError::InvalidQuery("corpo JSON inválido".to_string())
}

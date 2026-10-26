//! Histórico de audição do usuário autenticado (M2e).
//!
//! O histórico é sempre do `CurrentUser`: o cliente informa apenas a faixa, o
//! tipo do evento e (opcionalmente) os segundos ouvidos.

use std::collections::{HashMap, HashSet};

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use super::AppState;
use super::pagination::{Cursor, normalize_limit};
use crate::auth::CurrentUser;
use crate::domain::dto::{HistoryItem, HistoryPage, PlayEventCreated, TrackSummary};
use crate::error::ApiError;
use crate::repository::history::{self, EventKind};
use crate::repository::library;

/// Duração máxima aceita em `seconds` (24 h).
const MAX_SECONDS: f64 = 86_400.0;

/// Corpo de `POST /api/v1/me/history`.
#[derive(Debug, Deserialize)]
pub struct CreatePlayEventRequest {
    pub track_id: String,
    pub event_kind: String,
    pub seconds: Option<f64>,
}

/// Parâmetros de `GET /api/v1/me/history`.
#[derive(Debug, Deserialize)]
pub struct HistoryListParams {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

/// `POST /api/v1/me/history`
pub async fn create(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    payload: Result<Json<CreatePlayEventRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<PlayEventCreated>), ApiError> {
    let Json(payload) = payload.map_err(|_| invalid_body())?;

    let kind = EventKind::parse(payload.event_kind.trim()).ok_or_else(|| {
        ApiError::InvalidQuery(format!("event_kind inválido: '{}'", payload.event_kind))
    })?;

    if let Some(seconds) = payload.seconds
        && (!seconds.is_finite() || !(0.0..=MAX_SECONDS).contains(&seconds))
    {
        return Err(ApiError::InvalidQuery(
            "seconds deve estar entre 0 e 86400".to_string(),
        ));
    }

    let id = Uuid::now_v7().to_string();
    let inserted = history::insert_play_event(
        state.pool(),
        &id,
        &current.id,
        &payload.track_id,
        kind,
        payload.seconds,
    )
    .await
    .map_err(ApiError::Internal)?;

    if !inserted {
        return Err(ApiError::NotFound);
    }

    Ok((StatusCode::CREATED, Json(PlayEventCreated { id })))
}

/// `GET /api/v1/me/history`
pub async fn list(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Query(params): Query<HistoryListParams>,
) -> Result<Json<HistoryPage>, ApiError> {
    let limit = normalize_limit(params.limit);
    let cursor = params.cursor.as_deref().map(Cursor::decode).transpose()?;

    let (rows, next_cursor) =
        history::list_history(state.pool(), &current.id, cursor.as_ref(), limit)
            .await
            .map_err(ApiError::Internal)?;

    // Uma única consulta às faixas da página (sem N+1); ids repetidos contam uma vez.
    let mut seen = HashSet::with_capacity(rows.len());
    let mut track_ids = Vec::with_capacity(rows.len());
    for row in &rows {
        if seen.insert(row.track_id.as_str()) {
            track_ids.push(row.track_id.clone());
        }
    }

    let summaries = library::track_summaries_by_ids(state.pool(), &track_ids)
        .await
        .map_err(ApiError::Internal)?;
    let by_id: HashMap<String, TrackSummary> = summaries
        .into_iter()
        .map(|track| (track.id.clone(), track))
        .collect();

    // `get` (e não `remove`): a mesma faixa pode aparecer em vários eventos.
    let items = rows
        .into_iter()
        .filter_map(|row| {
            by_id.get(&row.track_id).cloned().map(|track| HistoryItem {
                id: row.id,
                track,
                event_kind: row.event_kind,
                seconds: row.seconds,
                occurred_at: row.occurred_at,
            })
        })
        .collect();

    Ok(Json(HistoryPage { items, next_cursor }))
}

/// `DELETE /api/v1/me/history`
pub async fn clear(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<StatusCode, ApiError> {
    history::clear_history(state.pool(), &current.id)
        .await
        .map_err(ApiError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}

fn invalid_body() -> ApiError {
    ApiError::InvalidQuery("corpo JSON inválido".to_string())
}

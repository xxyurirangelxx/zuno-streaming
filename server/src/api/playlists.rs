//! Playlists do usuário autenticado (M2c).
//!
//! Ownership é estrito: uma playlist de outro usuário responde 404, nunca 403,
//! para não revelar a existência do recurso.

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use super::AppState;
use crate::auth::CurrentUser;
use crate::domain::dto::{PlaylistDetail, PlaylistItemAdded, PlaylistSummary};
use crate::domain::normalize_key;
use crate::error::ApiError;
use crate::repository::library;
use crate::repository::playlists::{self, AddItemOutcome, RemoveItemOutcome, ReorderOutcome};

/// Comprimento máximo do nome de uma playlist.
const MAX_NAME_CHARS: usize = 80;

/// Corpo de `POST /api/v1/playlists`.
#[derive(Debug, Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
}

/// Corpo de `PATCH /api/v1/playlists/{id}`.
#[derive(Debug, Deserialize)]
pub struct UpdatePlaylistRequest {
    pub name: Option<String>,
    pub visibility: Option<String>,
}

/// Corpo de `POST /api/v1/playlists/{id}/items`.
#[derive(Debug, Deserialize)]
pub struct AddItemRequest {
    pub track_id: String,
}

/// Corpo de `PUT /api/v1/playlists/{id}/items/{track_id}/position`.
#[derive(Debug, Deserialize)]
pub struct ReorderItemRequest {
    pub position: i64,
}

/// `GET /api/v1/playlists`
pub async fn list(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<Vec<PlaylistSummary>>, ApiError> {
    let playlists = playlists::list_playlists(state.pool(), &current.id)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(playlists))
}

/// `POST /api/v1/playlists`
pub async fn create(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Json(payload): Json<CreatePlaylistRequest>,
) -> Result<(StatusCode, Json<PlaylistSummary>), ApiError> {
    let name = validate_name(&payload.name)?;
    let normalized_name = normalize_key(&name);

    if playlists::playlist_name_exists(state.pool(), &current.id, &normalized_name, None)
        .await
        .map_err(ApiError::Internal)?
    {
        return Err(duplicate_name());
    }

    let id = Uuid::now_v7().to_string();
    let created =
        playlists::create_playlist(state.pool(), &id, &current.id, &name, &normalized_name)
            .await
            .map_err(ApiError::Internal)?;

    if !created {
        return Err(duplicate_name());
    }

    let playlist = playlists::get_playlist(state.pool(), &current.id, &id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    Ok((StatusCode::CREATED, Json(playlist)))
}

/// `GET /api/v1/playlists/{id}`
pub async fn get(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(playlist_id): Path<String>,
) -> Result<Json<PlaylistDetail>, ApiError> {
    let header = playlists::get_playlist(state.pool(), &current.id, &playlist_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let track_ids = playlists::playlist_track_ids(state.pool(), &current.id, &playlist_id)
        .await
        .map_err(ApiError::Internal)?;
    let items = library::track_summaries_by_ids(state.pool(), &track_ids)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(PlaylistDetail {
        id: header.id,
        name: header.name,
        visibility: header.visibility,
        track_count: header.track_count,
        items,
    }))
}

/// `PATCH /api/v1/playlists/{id}`
pub async fn update(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(playlist_id): Path<String>,
    Json(payload): Json<UpdatePlaylistRequest>,
) -> Result<Json<PlaylistSummary>, ApiError> {
    let name = payload.name.as_deref().map(validate_name).transpose()?;
    let visibility = payload
        .visibility
        .as_deref()
        .map(validate_visibility)
        .transpose()?;

    // 404 antes de qualquer validação de conflito (não vaza existência alheia).
    playlists::get_playlist(state.pool(), &current.id, &playlist_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let normalized_name = name.as_deref().map(normalize_key);
    if let Some(normalized_name) = normalized_name.as_deref()
        && playlists::playlist_name_exists(
            state.pool(),
            &current.id,
            normalized_name,
            Some(&playlist_id),
        )
        .await
        .map_err(ApiError::Internal)?
    {
        return Err(duplicate_name());
    }

    let updated = playlists::update_playlist(
        state.pool(),
        &current.id,
        &playlist_id,
        name.as_deref(),
        normalized_name.as_deref(),
        visibility,
    )
    .await
    .map_err(ApiError::Internal)?;

    if !updated {
        return Err(ApiError::NotFound);
    }

    let playlist = playlists::get_playlist(state.pool(), &current.id, &playlist_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(playlist))
}

/// `DELETE /api/v1/playlists/{id}`
pub async fn delete(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(playlist_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = playlists::delete_playlist(state.pool(), &current.id, &playlist_id)
        .await
        .map_err(ApiError::Internal)?;

    if !deleted {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/playlists/{id}/items`
pub async fn add_item(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path(playlist_id): Path<String>,
    Json(payload): Json<AddItemRequest>,
) -> Result<(StatusCode, Json<PlaylistItemAdded>), ApiError> {
    let item_id = Uuid::now_v7().to_string();
    let outcome = playlists::add_playlist_item(
        state.pool(),
        &current.id,
        &playlist_id,
        &payload.track_id,
        &item_id,
    )
    .await
    .map_err(ApiError::Internal)?;

    match outcome {
        AddItemOutcome::Added(position) => {
            Ok((StatusCode::CREATED, Json(PlaylistItemAdded { position })))
        }
        AddItemOutcome::NotFound => Err(ApiError::NotFound),
        AddItemOutcome::Duplicate => Err(ApiError::Conflict("faixa já está na playlist")),
    }
}

/// `DELETE /api/v1/playlists/{id}/items/{track_id}`
pub async fn remove_item(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path((playlist_id, track_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let outcome =
        playlists::remove_playlist_item(state.pool(), &current.id, &playlist_id, &track_id)
            .await
            .map_err(ApiError::Internal)?;

    match outcome {
        RemoveItemOutcome::Removed => Ok(StatusCode::NO_CONTENT),
        RemoveItemOutcome::NotFound => Err(ApiError::NotFound),
    }
}

/// `PUT /api/v1/playlists/{id}/items/{track_id}/position`
pub async fn reorder_item(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path((playlist_id, track_id)): Path<(String, String)>,
    Json(payload): Json<ReorderItemRequest>,
) -> Result<StatusCode, ApiError> {
    let outcome = playlists::reorder_playlist_item(
        state.pool(),
        &current.id,
        &playlist_id,
        &track_id,
        payload.position,
    )
    .await
    .map_err(ApiError::Internal)?;

    match outcome {
        ReorderOutcome::Moved => Ok(StatusCode::NO_CONTENT),
        ReorderOutcome::NotFound => Err(ApiError::NotFound),
        ReorderOutcome::OutOfRange => Err(ApiError::InvalidQuery(
            "posição fora do intervalo da playlist".to_string(),
        )),
    }
}

fn validate_name(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    let length = trimmed.chars().count();

    if length == 0 {
        return Err(ApiError::InvalidQuery(
            "nome da playlist não pode ser vazio".to_string(),
        ));
    }

    if length > MAX_NAME_CHARS {
        return Err(ApiError::InvalidQuery(format!(
            "nome da playlist excede {MAX_NAME_CHARS} caracteres"
        )));
    }

    Ok(trimmed.to_string())
}

fn validate_visibility(raw: &str) -> Result<&'static str, ApiError> {
    match raw.trim() {
        "private" => Ok("private"),
        "unlisted" => Ok("unlisted"),
        other => Err(ApiError::InvalidQuery(format!(
            "visibility inválida: '{other}'"
        ))),
    }
}

fn duplicate_name() -> ApiError {
    ApiError::Conflict("já existe uma playlist com esse nome")
}

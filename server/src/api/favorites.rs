//! Favoritos do usuário autenticado (M2c).
//!
//! `PUT`/`DELETE` são idempotentes; o `GET` devolve os três grupos sempre
//! presentes (listas vazias quando não há favoritos).

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;

use super::AppState;
use crate::auth::CurrentUser;
use crate::domain::dto::FavoritesResponse;
use crate::error::ApiError;
use crate::repository::library;
use crate::repository::playlists::{self, FavoriteKind};

/// `GET /api/v1/favorites`
pub async fn list(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<FavoritesResponse>, ApiError> {
    let track_ids = playlists::list_favorites(state.pool(), &current.id, FavoriteKind::Track)
        .await
        .map_err(ApiError::Internal)?;
    let album_ids = playlists::list_favorites(state.pool(), &current.id, FavoriteKind::Album)
        .await
        .map_err(ApiError::Internal)?;
    let artist_ids = playlists::list_favorites(state.pool(), &current.id, FavoriteKind::Artist)
        .await
        .map_err(ApiError::Internal)?;

    let tracks = library::track_summaries_by_ids(state.pool(), &track_ids)
        .await
        .map_err(ApiError::Internal)?;
    let albums = library::album_summaries_by_ids(state.pool(), &album_ids)
        .await
        .map_err(ApiError::Internal)?;
    let artists = library::artist_summaries_by_ids(state.pool(), &artist_ids)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(FavoritesResponse {
        tracks,
        albums,
        artists,
    }))
}

/// `PUT /api/v1/favorites/{kind}/{id}`
pub async fn add(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path((kind, entity_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let kind = parse_kind(&kind)?;

    let added = playlists::add_favorite(state.pool(), &current.id, kind, &entity_id)
        .await
        .map_err(ApiError::Internal)?;

    if !added {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/v1/favorites/{kind}/{id}`
pub async fn remove(
    State(state): State<AppState>,
    Extension(current): Extension<CurrentUser>,
    Path((kind, entity_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let kind = parse_kind(&kind)?;

    playlists::remove_favorite(state.pool(), &current.id, kind, &entity_id)
        .await
        .map_err(ApiError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}

fn parse_kind(raw: &str) -> Result<FavoriteKind, ApiError> {
    FavoriteKind::parse(raw).ok_or_else(|| {
        ApiError::InvalidQuery(format!(
            "tipo de favorito inválido: '{raw}' (use track, album ou artist)"
        ))
    })
}

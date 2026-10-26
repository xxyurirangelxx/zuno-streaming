//! Endpoints de biblioteca: faixas, álbuns, artistas, busca e detalhe.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use super::AppState;
use super::pagination::{Cursor, normalize_limit};
use crate::error::ApiError;
use crate::repository::library::{self, TrackFilter};

/// Parâmetros de listagem de faixas.
#[derive(Debug, Deserialize)]
pub struct TrackListParams {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
    pub q: Option<String>,
}

/// Parâmetros de listagem de álbuns/artistas.
#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

/// Parâmetros de busca.
#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub limit: Option<i64>,
}

/// `GET /api/v1/tracks`
pub async fn list_tracks(
    State(state): State<AppState>,
    Query(params): Query<TrackListParams>,
) -> Result<Json<crate::domain::dto::TrackListResponse>, ApiError> {
    let limit = normalize_limit(params.limit);
    let cursor = params.cursor.as_deref().map(Cursor::decode).transpose()?;

    let filter = TrackFilter {
        album_id: params.album_id,
        artist_id: params.artist_id,
        query: params.q,
    };

    let (items, next_cursor) = library::list_tracks(state.pool(), &filter, cursor.as_ref(), limit)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(crate::domain::dto::TrackListResponse {
        items,
        next_cursor,
    }))
}

/// `GET /api/v1/tracks/{id}`
pub async fn get_track(
    State(state): State<AppState>,
    Path(track_id): Path<String>,
) -> Result<Json<crate::domain::dto::TrackDetail>, ApiError> {
    let detail = library::get_track_detail(state.pool(), &track_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(detail))
}

/// `GET /api/v1/albums`
pub async fn list_albums(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<crate::domain::dto::AlbumListResponse>, ApiError> {
    let limit = normalize_limit(params.limit);
    let cursor = params.cursor.as_deref().map(Cursor::decode).transpose()?;

    let (items, next_cursor) = library::list_albums(state.pool(), cursor.as_ref(), limit)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(crate::domain::dto::AlbumListResponse {
        items,
        next_cursor,
    }))
}

/// `GET /api/v1/artists`
pub async fn list_artists(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<crate::domain::dto::ArtistListResponse>, ApiError> {
    let limit = normalize_limit(params.limit);
    let cursor = params.cursor.as_deref().map(Cursor::decode).transpose()?;

    let (items, next_cursor) = library::list_artists(state.pool(), cursor.as_ref(), limit)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(crate::domain::dto::ArtistListResponse {
        items,
        next_cursor,
    }))
}

/// `GET /api/v1/search`
pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<crate::domain::dto::SearchResponse>, ApiError> {
    let query = params.q.unwrap_or_default();
    let trimmed = query.trim();

    if trimmed.is_empty() {
        return Ok(Json(crate::domain::dto::SearchResponse {
            query,
            tracks: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
        }));
    }

    let limit = normalize_limit(params.limit);
    let results = library::search_library(state.pool(), trimmed, limit)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(results))
}

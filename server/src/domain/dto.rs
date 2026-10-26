//! Contratos JSON da API v1 (M1b).
//!
//! Estes tipos são a superfície pública: mudá-los exige atualizar
//! `docs/api/openapi.yaml`, o cliente tipado e os testes de contrato.

use serde::{Deserialize, Serialize};

/// Referência mínima de artista dentro de uma faixa/álbum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistRef {
    pub id: String,
    pub name: String,
}

/// Referência mínima de álbum dentro de uma faixa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumRef {
    pub id: String,
    pub title: String,
}

/// Faixa como aparece em listagens e busca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackSummary {
    pub id: String,
    pub title: String,
    pub duration_ms: Option<u64>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub album: Option<AlbumRef>,
    pub artists: Vec<ArtistRef>,
    pub artwork_id: Option<String>,
    /// `false` quando nenhum arquivo disponível está indexado (nunca é removida).
    pub available: bool,
}

/// Álbum em listagens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumSummary {
    pub id: String,
    pub title: String,
    pub artist: Option<ArtistRef>,
    pub release_year: Option<i32>,
    pub artwork_id: Option<String>,
    pub track_count: i64,
}

/// Artista em listagens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistSummary {
    pub id: String,
    pub name: String,
    pub track_count: i64,
    pub album_count: i64,
}

/// Página de faixas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackListResponse {
    pub items: Vec<TrackSummary>,
    pub next_cursor: Option<String>,
}

/// Página de álbuns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumListResponse {
    pub items: Vec<AlbumSummary>,
    pub next_cursor: Option<String>,
}

/// Página de artistas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistListResponse {
    pub items: Vec<ArtistSummary>,
    pub next_cursor: Option<String>,
}

/// Resultado agregado de busca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub tracks: Vec<TrackSummary>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
}

/// Arquivo de áudio associado a uma faixa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFileInfo {
    pub id: String,
    pub codec: String,
    pub container: String,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub size_bytes: u64,
    pub available: bool,
}

/// Detalhe de uma faixa, incluindo o arquivo que será servido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackDetail {
    #[serde(flatten)]
    pub track: TrackSummary,
    pub audio: Option<AudioFileInfo>,
}

/// Playlist como aparece na listagem do dono.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistSummary {
    pub id: String,
    pub name: String,
    pub visibility: String,
    pub track_count: i64,
    pub updated_at: String,
}

/// Playlist com as faixas na ordem definida pelo dono.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistDetail {
    pub id: String,
    pub name: String,
    pub visibility: String,
    pub track_count: i64,
    pub items: Vec<TrackSummary>,
}

/// Posição devolvida ao adicionar uma faixa à playlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistItemAdded {
    pub position: i64,
}

/// Favoritos agrupados por tipo de entidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoritesResponse {
    pub tracks: Vec<TrackSummary>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
}

/// Preferências de interface do usuário autenticado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    pub theme: String,
    pub language: String,
}

/// Preferências de privacidade do usuário autenticado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Privacy {
    pub allow_cloud_history: bool,
    /// `None` significa que o histórico não expira.
    pub history_retention_days: Option<i64>,
}

/// Identificador do evento de audição recém-registrado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayEventCreated {
    pub id: String,
}

/// Item do histórico de audição do usuário, com a faixa já resolvida.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: String,
    pub track: TrackSummary,
    pub event_kind: String,
    pub seconds: Option<f64>,
    pub occurred_at: String,
}

/// Página do histórico de audição (keyset por `occurred_at DESC, id DESC`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryPage {
    pub items: Vec<HistoryItem>,
    pub next_cursor: Option<String>,
}

/// Faixa + dados necessários para resolver o streaming (uso interno).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamTarget {
    pub track_id: String,
    pub title: String,
    pub relative_path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub available: bool,
}

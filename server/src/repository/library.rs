//! Consultas de listagem, detalhe e busca da biblioteca (M1b).
//!
//! Regras:
//! * paginação por keyset (cursor opaco de `api::pagination`), nunca `OFFSET`;
//! * ordenação estável: `normalized_title, id` (faixas/álbuns) e `normalized_name, id` (artistas);
//! * uma página devolve `limit` itens e `next_cursor` só quando existe mais um;
//! * `available` reflete se há pelo menos um `audio_files.available = 1`;
//! * nada de N+1: uma consulta para a página e outra (com `IN (...)`) para os
//!   artistas das faixas da página.

use std::collections::HashMap;

use anyhow::{Context, Result};
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::api::pagination::Cursor;
use crate::domain::dto::{
    AlbumRef, AlbumSummary, ArtistRef, ArtistSummary, AudioFileInfo, SearchResponse, TrackDetail,
    TrackSummary,
};
use crate::domain::normalize_key;
use crate::repository::search_tracks;

/// Colunas da faixa já com o álbum resolvido e o cálculo de disponibilidade.
///
/// `available` usa `EXISTS` (semi-join) para não multiplicar linhas quando a
/// faixa tem mais de um arquivo indexado.
const TRACK_COLUMNS: &str = "t.id,
        t.normalized_title,
        t.title,
        t.duration_ms,
        t.track_no,
        t.disc_no,
        t.album_id,
        al.title AS album_title,
        al.artwork_id AS artwork_id,
        EXISTS (
            SELECT 1 FROM audio_files af WHERE af.track_id = t.id AND af.available = 1
        ) AS available";

/// `FROM` comum das consultas de faixa (o álbum é opcional).
const TRACK_FROM: &str = " FROM tracks t LEFT JOIN albums al ON al.id = t.album_id";

/// Papel usado ao expor artistas em `TrackSummary`.
///
/// A listagem mostra créditos de performance, na ordem definida pelo scanner.
const PERFORMER_ROLE: &str = "performer";

/// Filtros opcionais da listagem de faixas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackFilter {
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
    /// Busca textual opcional dentro da listagem.
    pub query: Option<String>,
}

/// Lista faixas com filtros e paginação por cursor.
pub async fn list_tracks(
    pool: &SqlitePool,
    filter: &TrackFilter,
    cursor: Option<&Cursor>,
    limit: i64,
) -> Result<(Vec<TrackSummary>, Option<String>)> {
    let page_size = usize::try_from(limit).context("limite de página inválido")?;
    let fetch = limit.saturating_add(1);

    let mut builder: QueryBuilder<'_, Sqlite> =
        QueryBuilder::new(format!("SELECT {TRACK_COLUMNS}{TRACK_FROM} WHERE 1 = 1"));

    if let Some(album_id) = filter.album_id.as_deref() {
        builder.push(" AND t.album_id = ").push_bind(album_id);
    }

    if let Some(artist_id) = filter.artist_id.as_deref() {
        // `EXISTS` evita duplicar a faixa quando o artista aparece em mais de um papel.
        builder
            .push(
                " AND EXISTS (
                    SELECT 1 FROM track_artists ta
                    WHERE ta.track_id = t.id AND ta.artist_id = ",
            )
            .push_bind(artist_id)
            .push(")");
    }

    if let Some(query) = filter.query.as_deref() {
        let Some(expression) = fts_prefix_expression(query) else {
            // Consulta sem nenhum token aproveitável (só pontuação/espaços).
            return Ok((Vec::new(), None));
        };

        // O FTS5 já remove diacríticos; o keyset continua valendo porque a
        // ordenação final é a da tabela `tracks` (título normalizado + id).
        builder
            .push(" AND t.id IN (SELECT track_id FROM tracks_fts WHERE tracks_fts MATCH ")
            .push_bind(expression)
            .push(")");
    }

    push_track_cursor(&mut builder, cursor);
    builder
        .push(" ORDER BY t.normalized_title, t.id LIMIT ")
        .push_bind(fetch);

    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("falha ao listar as faixas da biblioteca")?;

    let mut track_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        track_rows.push(track_row(row)?);
    }

    let has_more = track_rows.len() > page_size;
    track_rows.truncate(page_size);

    // O cursor vem da última linha incluída, nunca da linha extra descartada.
    let next_cursor = if has_more {
        track_rows
            .last()
            .map(|row| Cursor::encode(&row.normalized_title, &row.id))
    } else {
        None
    };

    let items = summaries_from_rows(pool, track_rows).await?;
    Ok((items, next_cursor))
}

/// Lista álbuns com paginação por cursor.
pub async fn list_albums(
    pool: &SqlitePool,
    cursor: Option<&Cursor>,
    limit: i64,
) -> Result<(Vec<AlbumSummary>, Option<String>)> {
    let page_size = usize::try_from(limit).context("limite de página inválido")?;
    let fetch = limit.saturating_add(1);

    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        "SELECT al.id,
                al.normalized_title,
                al.title,
                al.album_artist_id,
                al.release_year,
                al.artwork_id,
                ar.name AS artist_name,
                (SELECT COUNT(*) FROM tracks t WHERE t.album_id = al.id) AS track_count
         FROM albums al
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE 1 = 1",
    );

    if let Some(cursor) = cursor {
        builder
            .push(" AND (al.normalized_title > ")
            .push_bind(cursor.key.as_str())
            .push(" OR (al.normalized_title = ")
            .push_bind(cursor.key.as_str())
            .push(" AND al.id > ")
            .push_bind(cursor.id.as_str())
            .push("))");
    }

    builder
        .push(" ORDER BY al.normalized_title, al.id LIMIT ")
        .push_bind(fetch);

    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("falha ao listar os álbuns da biblioteca")?;

    let mut rows = rows;
    let has_more = rows.len() > page_size;
    rows.truncate(page_size);

    let mut items = Vec::with_capacity(rows.len());
    let mut last_key: Option<(String, String)> = None;

    for row in &rows {
        let artist_id: Option<String> = row.try_get("album_artist_id")?;
        let artist_name: Option<String> = row.try_get("artist_name")?;
        let artist = match (artist_id, artist_name) {
            (Some(id), Some(name)) => Some(ArtistRef { id, name }),
            _ => None,
        };

        let release_year: Option<i64> = row.try_get("release_year")?;
        let id: String = row.try_get("id")?;
        let normalized_title: String = row.try_get("normalized_title")?;

        items.push(AlbumSummary {
            id: id.clone(),
            title: row.try_get("title")?,
            artist,
            release_year: match release_year {
                Some(year) => Some(i32::try_from(year).context("release_year fora do intervalo")?),
                None => None,
            },
            artwork_id: row.try_get("artwork_id")?,
            track_count: row.try_get("track_count")?,
        });

        last_key = Some((normalized_title, id));
    }

    let next_cursor = if has_more {
        last_key.map(|(key, id)| Cursor::encode(&key, &id))
    } else {
        None
    };

    Ok((items, next_cursor))
}

/// Lista artistas com paginação por cursor.
///
/// Definições de contagem (documentadas aqui porque não são óbvias):
/// * `track_count` = faixas distintas em que o artista tem qualquer papel
///   (`track_artists`), não só `performer`;
/// * `album_count` = álbuns distintos em que o artista é `album_artist_id`
///   **ou** em que participa de pelo menos uma faixa (`UNION` deduplica).
pub async fn list_artists(
    pool: &SqlitePool,
    cursor: Option<&Cursor>,
    limit: i64,
) -> Result<(Vec<ArtistSummary>, Option<String>)> {
    let page_size = usize::try_from(limit).context("limite de página inválido")?;
    let fetch = limit.saturating_add(1);

    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        "SELECT ar.id,
                ar.normalized_name,
                ar.name,
                (SELECT COUNT(DISTINCT ta.track_id)
                 FROM track_artists ta
                 WHERE ta.artist_id = ar.id) AS track_count,
                (SELECT COUNT(*)
                 FROM (
                     SELECT al.id FROM albums al WHERE al.album_artist_id = ar.id
                     UNION
                     SELECT t.album_id
                     FROM track_artists ta
                     JOIN tracks t ON t.id = ta.track_id
                     WHERE ta.artist_id = ar.id AND t.album_id IS NOT NULL
                 )) AS album_count
         FROM artists ar
         WHERE 1 = 1",
    );

    if let Some(cursor) = cursor {
        builder
            .push(" AND (ar.normalized_name > ")
            .push_bind(cursor.key.as_str())
            .push(" OR (ar.normalized_name = ")
            .push_bind(cursor.key.as_str())
            .push(" AND ar.id > ")
            .push_bind(cursor.id.as_str())
            .push("))");
    }

    builder
        .push(" ORDER BY ar.normalized_name, ar.id LIMIT ")
        .push_bind(fetch);

    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("falha ao listar os artistas da biblioteca")?;

    let mut rows = rows;
    let has_more = rows.len() > page_size;
    rows.truncate(page_size);

    let mut items = Vec::with_capacity(rows.len());
    let mut last_key: Option<(String, String)> = None;

    for row in &rows {
        let id: String = row.try_get("id")?;
        let normalized_name: String = row.try_get("normalized_name")?;

        items.push(ArtistSummary {
            id: id.clone(),
            name: row.try_get("name")?,
            track_count: row.try_get("track_count")?,
            album_count: row.try_get("album_count")?,
        });

        last_key = Some((normalized_name, id));
    }

    let next_cursor = if has_more {
        last_key.map(|(key, id)| Cursor::encode(&key, &id))
    } else {
        None
    };

    Ok((items, next_cursor))
}

/// Busca agregada (faixas via FTS5, álbuns/artistas por prefixo normalizado).
///
/// As faixas vêm do FTS (`repository::search_tracks`, ordenado por relevância) e
/// os resumos são carregados preservando essa ordem. Álbuns e artistas são
/// casados por prefixo no campo normalizado (sem acento no `normalized_*`, que é
/// minúsculo); consultas com acento dependem do FTS, que remove diacríticos.
pub async fn search_library(pool: &SqlitePool, query: &str, limit: i64) -> Result<SearchResponse> {
    let trimmed = query.trim();

    if trimmed.is_empty() {
        return Ok(SearchResponse {
            query: query.to_string(),
            tracks: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
        });
    }

    let track_ids = search_tracks(pool, trimmed, limit)
        .await
        .context("falha ao buscar as faixas no índice full-text")?;
    let tracks = summaries_by_ids(pool, &track_ids).await?;
    let albums = search_albums(pool, trimmed, limit).await?;
    let artists = search_artists(pool, trimmed, limit).await?;

    Ok(SearchResponse {
        query: trimmed.to_string(),
        tracks,
        albums,
        artists,
    })
}

/// Detalhe de uma faixa, incluindo o arquivo associado (se houver).
pub async fn get_track_detail(pool: &SqlitePool, track_id: &str) -> Result<Option<TrackDetail>> {
    let row = sqlx::query(&format!(
        "SELECT {TRACK_COLUMNS}{TRACK_FROM} WHERE t.id = ?1"
    ))
    .bind(track_id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("falha ao carregar a faixa '{track_id}'"))?;

    let Some(row) = row else {
        return Ok(None);
    };

    let track_row = track_row(&row)?;
    let mut summaries = summaries_from_rows(pool, vec![track_row]).await?;
    let Some(track) = summaries.pop() else {
        return Ok(None);
    };

    let audio = load_preferred_audio_file(pool, track_id).await?;

    Ok(Some(TrackDetail { track, audio }))
}

// ---------------------------------------------------------------------------
// Helpers privados
// ---------------------------------------------------------------------------

/// Linha de faixa no formato usado por listagens/detalhe.
struct TrackRow {
    id: String,
    normalized_title: String,
    title: String,
    duration_ms: Option<i64>,
    track_no: Option<i64>,
    disc_no: Option<i64>,
    album_id: Option<String>,
    album_title: Option<String>,
    artwork_id: Option<String>,
    available: bool,
}

fn track_row(row: &SqliteRow) -> Result<TrackRow> {
    let available: i64 = row.try_get("available")?;

    Ok(TrackRow {
        id: row.try_get("id").context("coluna `id` da faixa")?,
        normalized_title: row
            .try_get("normalized_title")
            .context("coluna `normalized_title` da faixa")?,
        title: row.try_get("title").context("coluna `title` da faixa")?,
        duration_ms: row
            .try_get("duration_ms")
            .context("coluna `duration_ms` da faixa")?,
        track_no: row
            .try_get("track_no")
            .context("coluna `track_no` da faixa")?,
        disc_no: row
            .try_get("disc_no")
            .context("coluna `disc_no` da faixa")?,
        album_id: row
            .try_get("album_id")
            .context("coluna `album_id` da faixa")?,
        album_title: row
            .try_get("album_title")
            .context("coluna `album_title` da faixa")?,
        artwork_id: row
            .try_get("artwork_id")
            .context("coluna `artwork_id` do álbum")?,
        available: available != 0,
    })
}

/// Converte a página de linhas em `TrackSummary` (uma consulta extra para artistas).
async fn summaries_from_rows(pool: &SqlitePool, rows: Vec<TrackRow>) -> Result<Vec<TrackSummary>> {
    let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
    let mut performers = load_performers(pool, &ids).await?;

    rows.into_iter()
        .map(|row| {
            let artists = performers.remove(&row.id).unwrap_or_default();
            to_summary(row, artists)
        })
        .collect()
}

fn to_summary(row: TrackRow, artists: Vec<ArtistRef>) -> Result<TrackSummary> {
    let album = match (row.album_id, row.album_title) {
        (Some(id), Some(title)) => Some(AlbumRef { id, title }),
        _ => None,
    };

    Ok(TrackSummary {
        id: row.id,
        title: row.title,
        duration_ms: optional_u64(row.duration_ms, "duration_ms")?,
        track_no: optional_u32(row.track_no, "track_no")?,
        disc_no: optional_u32(row.disc_no, "disc_no")?,
        album,
        artists,
        artwork_id: row.artwork_id,
        available: row.available,
    })
}

/// Carrega os artistas de performance das faixas informadas em uma única consulta.
async fn load_performers(
    pool: &SqlitePool,
    track_ids: &[&str],
) -> Result<HashMap<String, Vec<ArtistRef>>> {
    if track_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(format!(
        "SELECT ta.track_id, ar.id AS artist_id, ar.name AS artist_name
         FROM track_artists ta
         JOIN artists ar ON ar.id = ta.artist_id
         WHERE ta.role = '{PERFORMER_ROLE}' AND ta.track_id IN ("
    ));

    {
        let mut separated = builder.separated(", ");
        for track_id in track_ids {
            separated.push_bind(*track_id);
        }
    }

    builder.push(") ORDER BY ta.track_id, ta.sort_order, ar.name");

    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("falha ao carregar os artistas das faixas")?;

    let mut map: HashMap<String, Vec<ArtistRef>> = HashMap::new();
    for row in &rows {
        let track_id: String = row.try_get("track_id")?;
        let artist = ArtistRef {
            id: row.try_get("artist_id")?,
            name: row.try_get("artist_name")?,
        };
        map.entry(track_id).or_default().push(artist);
    }

    Ok(map)
}

/// Carrega os resumos das faixas pelos ids informados, preservando a ordem dada
/// (usado pela busca, cuja ordem é a relevância do FTS).
async fn summaries_by_ids(pool: &SqlitePool, ids: &[String]) -> Result<Vec<TrackSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(format!(
        "SELECT {TRACK_COLUMNS}{TRACK_FROM} WHERE t.id IN ("
    ));
    {
        let mut separated = builder.separated(", ");
        for id in ids {
            separated.push_bind(id.as_str());
        }
    }
    builder.push(")");

    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .context("falha ao carregar as faixas encontradas")?;

    let mut by_id: HashMap<String, TrackRow> = HashMap::with_capacity(rows.len());
    for row in &rows {
        let row = track_row(row)?;
        by_id.insert(row.id.clone(), row);
    }

    let mut ordered = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(row) = by_id.remove(id) {
            ordered.push(row);
        }
    }

    summaries_from_rows(pool, ordered).await
}

/// Álbuns cujo título normalizado começa pela consulta.
async fn search_albums(pool: &SqlitePool, query: &str, limit: i64) -> Result<Vec<AlbumSummary>> {
    let pattern = like_prefix(query);

    let rows = sqlx::query(
        "SELECT al.id,
                al.title,
                al.album_artist_id,
                al.release_year,
                al.artwork_id,
                ar.name AS artist_name,
                (SELECT COUNT(*) FROM tracks t WHERE t.album_id = al.id) AS track_count
         FROM albums al
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE al.normalized_title LIKE ?1 ESCAPE '\\'
         ORDER BY al.normalized_title, al.id
         LIMIT ?2",
    )
    .bind(pattern)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("falha ao buscar álbuns por prefixo")?;

    let mut albums = Vec::with_capacity(rows.len());
    for row in &rows {
        let artist_id: Option<String> = row.try_get("album_artist_id")?;
        let artist_name: Option<String> = row.try_get("artist_name")?;
        let artist = match (artist_id, artist_name) {
            (Some(id), Some(name)) => Some(ArtistRef { id, name }),
            _ => None,
        };
        let release_year: Option<i64> = row.try_get("release_year")?;

        albums.push(AlbumSummary {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            artist,
            release_year: match release_year {
                Some(year) => Some(i32::try_from(year).context("release_year fora do intervalo")?),
                None => None,
            },
            artwork_id: row.try_get("artwork_id")?,
            track_count: row.try_get("track_count")?,
        });
    }

    Ok(albums)
}

/// Artistas cujo nome normalizado começa pela consulta.
async fn search_artists(pool: &SqlitePool, query: &str, limit: i64) -> Result<Vec<ArtistSummary>> {
    let pattern = like_prefix(query);

    let rows = sqlx::query(
        "SELECT ar.id,
                ar.name,
                (SELECT COUNT(DISTINCT ta.track_id)
                 FROM track_artists ta
                 WHERE ta.artist_id = ar.id) AS track_count,
                (SELECT COUNT(*)
                 FROM (
                     SELECT al.id FROM albums al WHERE al.album_artist_id = ar.id
                     UNION
                     SELECT t.album_id
                     FROM track_artists ta
                     JOIN tracks t ON t.id = ta.track_id
                     WHERE ta.artist_id = ar.id AND t.album_id IS NOT NULL
                 )) AS album_count
         FROM artists ar
         WHERE ar.normalized_name LIKE ?1 ESCAPE '\\'
         ORDER BY ar.normalized_name, ar.id
         LIMIT ?2",
    )
    .bind(pattern)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("falha ao buscar artistas por prefixo")?;

    let mut artists = Vec::with_capacity(rows.len());
    for row in &rows {
        artists.push(ArtistSummary {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            track_count: row.try_get("track_count")?,
            album_count: row.try_get("album_count")?,
        });
    }

    Ok(artists)
}

/// Arquivo que será servido: disponível primeiro, depois o último indexado.
async fn load_preferred_audio_file(
    pool: &SqlitePool,
    track_id: &str,
) -> Result<Option<AudioFileInfo>> {
    let row = sqlx::query(
        "SELECT id, codec, container, bitrate, sample_rate, channels, size_bytes, available
         FROM audio_files
         WHERE track_id = ?1
         ORDER BY available DESC, scanned_at DESC, id
         LIMIT 1",
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("falha ao carregar o arquivo de áudio da faixa '{track_id}'"))?;

    let Some(row) = row else {
        return Ok(None);
    };

    let bitrate: Option<i64> = row.try_get("bitrate")?;
    let sample_rate: Option<i64> = row.try_get("sample_rate")?;
    let channels: Option<i64> = row.try_get("channels")?;
    let size_bytes: i64 = row.try_get("size_bytes")?;
    let available: i64 = row.try_get("available")?;

    Ok(Some(AudioFileInfo {
        id: row.try_get("id")?,
        codec: row.try_get("codec")?,
        container: row.try_get("container")?,
        bitrate: optional_u32(bitrate, "bitrate")?,
        sample_rate: optional_u32(sample_rate, "sample_rate")?,
        channels: match optional_u32(channels, "channels")? {
            Some(value) => Some(u8::try_from(value).context("channels fora do intervalo")?),
            None => None,
        },
        size_bytes: u64::try_from(size_bytes).context("size_bytes não pode ser negativo")?,
        available: available != 0,
    }))
}

fn push_track_cursor<'q>(builder: &mut QueryBuilder<'q, Sqlite>, cursor: Option<&'q Cursor>) {
    if let Some(cursor) = cursor {
        builder
            .push(" AND (t.normalized_title > ")
            .push_bind(cursor.key.as_str())
            .push(" OR (t.normalized_title = ")
            .push_bind(cursor.key.as_str())
            .push(" AND t.id > ")
            .push_bind(cursor.id.as_str())
            .push("))");
    }
}

/// Monta a expressão FTS5 a partir da consulta do usuário.
///
/// Mantém apenas letras, números e espaços (descarta a sintaxe do FTS5) e
/// transforma cada token em prefixo. Espelha `repository::build_match_expression`
/// (privado) para permitir filtrar dentro da mesma consulta paginada.
fn fts_prefix_expression(query: &str) -> Option<String> {
    let sanitized: String = query
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect();

    let tokens: Vec<String> = sanitized
        .split_whitespace()
        .map(|token| format!("{token}*"))
        .collect();

    if tokens.is_empty() {
        return None;
    }

    Some(tokens.join(" AND "))
}

/// Padrão `prefixo%` para `LIKE`, escapando curingas digitados pelo usuário.
fn like_prefix(query: &str) -> String {
    let normalized = normalize_key(query);

    let mut pattern = String::with_capacity(normalized.len() + 1);
    for character in normalized.chars() {
        if matches!(character, '\\' | '%' | '_') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push('%');

    pattern
}

fn optional_u64(value: Option<i64>, column: &str) -> Result<Option<u64>> {
    match value {
        Some(value) => {
            Ok(Some(u64::try_from(value).with_context(|| {
                format!("coluna `{column}` não pode ser negativa")
            })?))
        }
        None => Ok(None),
    }
}

fn optional_u32(value: Option<i64>, column: &str) -> Result<Option<u32>> {
    match value {
        Some(value) => Ok(Some(u32::try_from(value).with_context(|| {
            format!("coluna `{column}` fora do intervalo de 32 bits")
        })?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts_expression_uses_prefix_tokens() {
        assert_eq!(
            fts_prefix_expression("  Caetano   Veloso "),
            Some("Caetano* AND Veloso*".to_string())
        );
        assert_eq!(fts_prefix_expression("-\"\" "), None);
    }

    #[test]
    fn like_prefix_escapes_wildcards() {
        assert_eq!(like_prefix("  Chega   de Saudade "), "chega de saudade%");
        assert_eq!(like_prefix("100%"), "100\\%%");
        assert_eq!(like_prefix("a_b"), "a\\_b%");
    }
}

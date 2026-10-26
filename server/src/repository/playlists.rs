//! Playlists e favoritos (M2c).
//!
//! Regras:
//! * toda consulta filtra por `owner_id`/`user_id`; o cliente nunca escolhe o dono;
//! * playlist de outro usuário é tratada como inexistente (o handler devolve 404);
//! * mutações que tocam várias linhas (adicionar/remover/reordenar) usam transação;
//! * `updated_at` e `version` são atualizados em toda mutação da playlist;
//! * SQL sempre parametrizado (`QueryBuilder`/`?n`).

use anyhow::{Context, Result};
use sqlx::sqlite::SqliteConnection;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::domain::dto::PlaylistSummary;

/// Expressão SQLite de "agora" no formato ISO-8601 UTC (mesmo padrão das migrations).
const NOW_SQL: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

/// Resultado de `add_playlist_item`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddItemOutcome {
    /// Faixa inserida na posição informada.
    Added(i64),
    /// Playlist não pertence ao usuário ou faixa inexistente.
    NotFound,
    /// A faixa já está na playlist.
    Duplicate,
}

/// Resultado de `remove_playlist_item`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveItemOutcome {
    Removed,
    NotFound,
}

/// Resultado de `reorder_playlist_item`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReorderOutcome {
    Moved,
    NotFound,
    /// Posição fora de `0..len-1`.
    OutOfRange,
}

/// Tipo de entidade favoritável.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavoriteKind {
    Track,
    Album,
    Artist,
}

impl FavoriteKind {
    /// Converte o segmento de rota em tipo de entidade.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "track" => Some(Self::Track),
            "album" => Some(Self::Album),
            "artist" => Some(Self::Artist),
            _ => None,
        }
    }

    /// Valor persistido em `favorites.entity_kind`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Album => "album",
            Self::Artist => "artist",
        }
    }

    /// Consulta que confirma a existência da entidade alvo.
    fn exists_sql(self) -> &'static str {
        match self {
            Self::Track => "SELECT id FROM tracks WHERE id = ?1",
            Self::Album => "SELECT id FROM albums WHERE id = ?1",
            Self::Artist => "SELECT id FROM artists WHERE id = ?1",
        }
    }
}

/// Lista as playlists do usuário, mais recentes primeiro.
pub async fn list_playlists(pool: &SqlitePool, owner_id: &str) -> Result<Vec<PlaylistSummary>> {
    let rows = sqlx::query(
        "SELECT p.id,
                p.name,
                p.visibility,
                (SELECT COUNT(*) FROM playlist_items pi WHERE pi.playlist_id = p.id) AS track_count,
                p.updated_at
         FROM playlists p
         WHERE p.owner_id = ?1
         ORDER BY p.updated_at DESC, p.id",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
    .context("falha ao listar as playlists do usuário")?;

    rows.iter().map(playlist_summary).collect()
}

/// Carrega a playlist do usuário (404 quando não existe ou pertence a outro).
pub async fn get_playlist(
    pool: &SqlitePool,
    owner_id: &str,
    playlist_id: &str,
) -> Result<Option<PlaylistSummary>> {
    let row = sqlx::query(
        "SELECT p.id,
                p.name,
                p.visibility,
                (SELECT COUNT(*) FROM playlist_items pi WHERE pi.playlist_id = p.id) AS track_count,
                p.updated_at
         FROM playlists p
         WHERE p.id = ?1 AND p.owner_id = ?2",
    )
    .bind(playlist_id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("falha ao carregar a playlist '{playlist_id}'"))?;

    row.as_ref().map(playlist_summary).transpose()
}

/// Indica se o usuário já tem uma playlist com o nome normalizado.
///
/// `excluding_id` permite validar um `PATCH` sem conflitar com a própria playlist.
pub async fn playlist_name_exists(
    pool: &SqlitePool,
    owner_id: &str,
    normalized_name: &str,
    excluding_id: Option<&str>,
) -> Result<bool> {
    let found: Option<String> = sqlx::query_scalar(
        "SELECT id FROM playlists
         WHERE owner_id = ?1 AND normalized_name = ?2 AND (?3 IS NULL OR id <> ?3)",
    )
    .bind(owner_id)
    .bind(normalized_name)
    .bind(excluding_id)
    .fetch_optional(pool)
    .await
    .context("falha ao verificar o nome da playlist")?;

    Ok(found.is_some())
}

/// Insere a playlist. Devolve `false` quando o nome já existe para o usuário.
pub async fn create_playlist(
    pool: &SqlitePool,
    id: &str,
    owner_id: &str,
    name: &str,
    normalized_name: &str,
) -> Result<bool> {
    let result = sqlx::query(
        "INSERT INTO playlists (id, owner_id, name, normalized_name)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (owner_id, normalized_name) DO NOTHING",
    )
    .bind(id)
    .bind(owner_id)
    .bind(name)
    .bind(normalized_name)
    .execute(pool)
    .await
    .with_context(|| format!("falha ao criar a playlist '{name}'"))?;

    Ok(result.rows_affected() > 0)
}

/// Atualiza nome/visibilidade da playlist do usuário.
///
/// Devolve `false` quando não existe para esse dono.
pub async fn update_playlist(
    pool: &SqlitePool,
    owner_id: &str,
    playlist_id: &str,
    name: Option<&str>,
    normalized_name: Option<&str>,
    visibility: Option<&str>,
) -> Result<bool> {
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(format!(
        "UPDATE playlists SET updated_at = {NOW_SQL}, version = version + 1"
    ));

    if let Some(name) = name {
        builder.push(", name = ").push_bind(name);
        builder
            .push(", normalized_name = ")
            .push_bind(normalized_name.unwrap_or(name));
    }

    if let Some(visibility) = visibility {
        builder.push(", visibility = ").push_bind(visibility);
    }

    builder
        .push(" WHERE id = ")
        .push_bind(playlist_id)
        .push(" AND owner_id = ")
        .push_bind(owner_id);

    let result = builder
        .build()
        .execute(pool)
        .await
        .with_context(|| format!("falha ao atualizar a playlist '{playlist_id}'"))?;

    Ok(result.rows_affected() > 0)
}

/// Apaga a playlist do usuário (itens saem por `ON DELETE CASCADE`).
pub async fn delete_playlist(pool: &SqlitePool, owner_id: &str, playlist_id: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM playlists WHERE id = ?1 AND owner_id = ?2")
        .bind(playlist_id)
        .bind(owner_id)
        .execute(pool)
        .await
        .with_context(|| format!("falha ao apagar a playlist '{playlist_id}'"))?;

    Ok(result.rows_affected() > 0)
}

/// IDs das faixas da playlist, na ordem de `position`.
///
/// O `JOIN` com `playlists` mantém o filtro por dono nesta consulta.
pub async fn playlist_track_ids(
    pool: &SqlitePool,
    owner_id: &str,
    playlist_id: &str,
) -> Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT pi.track_id
         FROM playlist_items pi
         JOIN playlists p ON p.id = pi.playlist_id
         WHERE pi.playlist_id = ?1 AND p.owner_id = ?2
         ORDER BY pi.position, pi.id",
    )
    .bind(playlist_id)
    .bind(owner_id)
    .fetch_all(pool)
    .await
    .with_context(|| format!("falha ao listar as faixas da playlist '{playlist_id}'"))
}

/// Adiciona uma faixa ao final da playlist.
pub async fn add_playlist_item(
    pool: &SqlitePool,
    owner_id: &str,
    playlist_id: &str,
    track_id: &str,
    item_id: &str,
) -> Result<AddItemOutcome> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;

    if !owns_playlist(&mut tx, owner_id, playlist_id).await? {
        return Ok(AddItemOutcome::NotFound);
    }

    let track_exists: Option<String> = sqlx::query_scalar("SELECT id FROM tracks WHERE id = ?1")
        .bind(track_id)
        .fetch_optional(&mut *tx)
        .await
        .with_context(|| format!("falha ao verificar a faixa '{track_id}'"))?;

    if track_exists.is_none() {
        return Ok(AddItemOutcome::NotFound);
    }

    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT position FROM playlist_items WHERE playlist_id = ?1 AND track_id = ?2",
    )
    .bind(playlist_id)
    .bind(track_id)
    .fetch_optional(&mut *tx)
    .await
    .context("falha ao verificar a faixa na playlist")?;

    if existing.is_some() {
        return Ok(AddItemOutcome::Duplicate);
    }

    let last_position: Option<i64> =
        sqlx::query_scalar("SELECT MAX(position) FROM playlist_items WHERE playlist_id = ?1")
            .bind(playlist_id)
            .fetch_one(&mut *tx)
            .await
            .context("falha ao calcular a posição da faixa na playlist")?;

    let position = last_position.map_or(0, |value| value.saturating_add(1));

    let inserted = sqlx::query(
        "INSERT INTO playlist_items (id, playlist_id, track_id, position)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (playlist_id, track_id) DO NOTHING",
    )
    .bind(item_id)
    .bind(playlist_id)
    .bind(track_id)
    .bind(position)
    .execute(&mut *tx)
    .await
    .with_context(|| format!("falha ao adicionar a faixa '{track_id}' à playlist"))?;

    if inserted.rows_affected() == 0 {
        return Ok(AddItemOutcome::Duplicate);
    }

    touch_playlist(&mut tx, playlist_id).await?;
    tx.commit().await.context("falha ao confirmar transação")?;

    Ok(AddItemOutcome::Added(position))
}

/// Remove a faixa da playlist e recompacta as posições restantes.
pub async fn remove_playlist_item(
    pool: &SqlitePool,
    owner_id: &str,
    playlist_id: &str,
    track_id: &str,
) -> Result<RemoveItemOutcome> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;

    if !owns_playlist(&mut tx, owner_id, playlist_id).await? {
        return Ok(RemoveItemOutcome::NotFound);
    }

    let position: Option<i64> = sqlx::query_scalar(
        "SELECT position FROM playlist_items WHERE playlist_id = ?1 AND track_id = ?2",
    )
    .bind(playlist_id)
    .bind(track_id)
    .fetch_optional(&mut *tx)
    .await
    .context("falha ao localizar a faixa na playlist")?;

    let Some(position) = position else {
        return Ok(RemoveItemOutcome::NotFound);
    };

    sqlx::query("DELETE FROM playlist_items WHERE playlist_id = ?1 AND track_id = ?2")
        .bind(playlist_id)
        .bind(track_id)
        .execute(&mut *tx)
        .await
        .context("falha ao remover a faixa da playlist")?;

    // Recompacta 0..n-1: tudo depois da posição removida desce um.
    sqlx::query(
        "UPDATE playlist_items SET position = position - 1
         WHERE playlist_id = ?1 AND position > ?2",
    )
    .bind(playlist_id)
    .bind(position)
    .execute(&mut *tx)
    .await
    .context("falha ao recompactar as posições da playlist")?;

    touch_playlist(&mut tx, playlist_id).await?;
    tx.commit().await.context("falha ao confirmar transação")?;

    Ok(RemoveItemOutcome::Removed)
}

/// Move a faixa para `position` (0..len-1), reescrevendo a ordem completa.
pub async fn reorder_playlist_item(
    pool: &SqlitePool,
    owner_id: &str,
    playlist_id: &str,
    track_id: &str,
    position: i64,
) -> Result<ReorderOutcome> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;

    if !owns_playlist(&mut tx, owner_id, playlist_id).await? {
        return Ok(ReorderOutcome::NotFound);
    }

    let mut track_ids: Vec<String> = sqlx::query_scalar(
        "SELECT track_id FROM playlist_items
         WHERE playlist_id = ?1
         ORDER BY position, id",
    )
    .bind(playlist_id)
    .fetch_all(&mut *tx)
    .await
    .context("falha ao carregar a ordem da playlist")?;

    if track_ids.is_empty() {
        return Ok(ReorderOutcome::NotFound);
    }

    let Some(current) = track_ids.iter().position(|id| id == track_id) else {
        return Ok(ReorderOutcome::NotFound);
    };

    let len = i64::try_from(track_ids.len()).context("playlist grande demais")?;
    if position < 0 || position >= len {
        return Ok(ReorderOutcome::OutOfRange);
    }

    let target = usize::try_from(position).context("posição inválida")?;
    if current != target {
        let moved = track_ids.remove(current);
        track_ids.insert(target, moved);
    }

    for (index, id) in track_ids.iter().enumerate() {
        let index = i64::try_from(index).context("posição inválida")?;
        sqlx::query(
            "UPDATE playlist_items SET position = ?3
             WHERE playlist_id = ?1 AND track_id = ?2",
        )
        .bind(playlist_id)
        .bind(id)
        .bind(index)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("falha ao mover a faixa '{id}' na playlist"))?;
    }

    touch_playlist(&mut tx, playlist_id).await?;
    tx.commit().await.context("falha ao confirmar transação")?;

    Ok(ReorderOutcome::Moved)
}

/// IDs favoritos do usuário para um tipo, na ordem em que foram marcados.
pub async fn list_favorites(
    pool: &SqlitePool,
    user_id: &str,
    kind: FavoriteKind,
) -> Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT entity_id FROM favorites
         WHERE user_id = ?1 AND entity_kind = ?2
         ORDER BY created_at, entity_id",
    )
    .bind(user_id)
    .bind(kind.as_str())
    .fetch_all(pool)
    .await
    .with_context(|| format!("falha ao listar favoritos do tipo '{}'", kind.as_str()))
}

/// Marca o favorito (idempotente). Devolve `false` quando a entidade não existe.
pub async fn add_favorite(
    pool: &SqlitePool,
    user_id: &str,
    kind: FavoriteKind,
    entity_id: &str,
) -> Result<bool> {
    let exists: Option<String> = sqlx::query_scalar(kind.exists_sql())
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("falha ao verificar a entidade favorita '{entity_id}'"))?;

    if exists.is_none() {
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO favorites (user_id, entity_kind, entity_id)
         VALUES (?1, ?2, ?3)
         ON CONFLICT (user_id, entity_kind, entity_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(kind.as_str())
    .bind(entity_id)
    .execute(pool)
    .await
    .with_context(|| format!("falha ao favoritar a entidade '{entity_id}'"))?;

    Ok(true)
}

/// Remove o favorito (idempotente).
pub async fn remove_favorite(
    pool: &SqlitePool,
    user_id: &str,
    kind: FavoriteKind,
    entity_id: &str,
) -> Result<()> {
    sqlx::query("DELETE FROM favorites WHERE user_id = ?1 AND entity_kind = ?2 AND entity_id = ?3")
        .bind(user_id)
        .bind(kind.as_str())
        .bind(entity_id)
        .execute(pool)
        .await
        .with_context(|| format!("falha ao remover o favorito '{entity_id}'"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers privados
// ---------------------------------------------------------------------------

fn playlist_summary(row: &sqlx::sqlite::SqliteRow) -> Result<PlaylistSummary> {
    Ok(PlaylistSummary {
        id: row.try_get("id").context("coluna `id` da playlist")?,
        name: row.try_get("name").context("coluna `name` da playlist")?,
        visibility: row
            .try_get("visibility")
            .context("coluna `visibility` da playlist")?,
        track_count: row
            .try_get("track_count")
            .context("coluna `track_count` da playlist")?,
        updated_at: row
            .try_get("updated_at")
            .context("coluna `updated_at` da playlist")?,
    })
}

async fn owns_playlist(
    conn: &mut SqliteConnection,
    owner_id: &str,
    playlist_id: &str,
) -> Result<bool> {
    let found: Option<String> =
        sqlx::query_scalar("SELECT id FROM playlists WHERE id = ?1 AND owner_id = ?2")
            .bind(playlist_id)
            .bind(owner_id)
            .fetch_optional(&mut *conn)
            .await
            .with_context(|| format!("falha ao verificar a playlist '{playlist_id}'"))?;

    Ok(found.is_some())
}

async fn touch_playlist(conn: &mut SqliteConnection, playlist_id: &str) -> Result<()> {
    sqlx::query(&format!(
        "UPDATE playlists SET updated_at = {NOW_SQL}, version = version + 1 WHERE id = ?1"
    ))
    .bind(playlist_id)
    .execute(&mut *conn)
    .await
    .with_context(|| format!("falha ao atualizar o timestamp da playlist '{playlist_id}'"))?;

    Ok(())
}

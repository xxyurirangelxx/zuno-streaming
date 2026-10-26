//! Histórico de audição do usuário (M2e).
//!
//! Regras:
//! * todo evento pertence ao usuário autenticado (`user_id` vem do `CurrentUser`);
//! * `play_events` é append-only; `DELETE` apaga somente as linhas do próprio
//!   usuário;
//! * a listagem usa keyset (`occurred_at DESC, id DESC`) com cursor opaco de
//!   `api::pagination` — nunca `OFFSET`;
//! * inserir evento de faixa inexistente devolve `false` (o handler responde 404);
//! * SQL sempre parametrizado.

use anyhow::{Context, Result};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use crate::api::pagination::Cursor;

/// Tipo de evento de audição registrado em `play_events.event_kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Play,
    Skip,
    Complete,
}

impl EventKind {
    /// Converte o valor recebido do cliente.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "play" => Some(Self::Play),
            "skip" => Some(Self::Skip),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }

    /// Valor persistido em `play_events.event_kind`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Play => "play",
            Self::Skip => "skip",
            Self::Complete => "complete",
        }
    }
}

/// Linha do histórico já ordenada (a faixa é resolvida pelo handler via `library`).
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryRow {
    pub id: String,
    pub track_id: String,
    pub event_kind: String,
    pub seconds: Option<f64>,
    pub occurred_at: String,
}

/// Registra o evento de audição do usuário.
///
/// Devolve `false` quando a faixa não existe (nenhuma linha é gravada).
pub async fn insert_play_event(
    pool: &SqlitePool,
    id: &str,
    user_id: &str,
    track_id: &str,
    kind: EventKind,
    seconds: Option<f64>,
) -> Result<bool> {
    let mut tx = pool.begin().await.context("falha ao abrir transação")?;

    let track_exists: Option<String> = sqlx::query_scalar("SELECT id FROM tracks WHERE id = ?1")
        .bind(track_id)
        .fetch_optional(&mut *tx)
        .await
        .with_context(|| format!("falha ao verificar a faixa '{track_id}'"))?;

    if track_exists.is_none() {
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO play_events (id, user_id, track_id, event_kind, seconds)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(id)
    .bind(user_id)
    .bind(track_id)
    .bind(kind.as_str())
    .bind(seconds)
    .execute(&mut *tx)
    .await
    .with_context(|| format!("falha ao registrar o evento '{kind:?}' da faixa '{track_id}'"))?;

    tx.commit().await.context("falha ao confirmar transação")?;

    Ok(true)
}

/// Lista o histórico do usuário, mais recente primeiro, com paginação keyset.
///
/// O cursor carrega `(occurred_at, id)` da última linha incluída: a próxima
/// página começa estritamente depois desse par na ordenação decrescente.
pub async fn list_history(
    pool: &SqlitePool,
    user_id: &str,
    cursor: Option<&Cursor>,
    limit: i64,
) -> Result<(Vec<HistoryRow>, Option<String>)> {
    let page_size = usize::try_from(limit).context("limite de página inválido")?;
    let fetch = limit.saturating_add(1);

    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        "SELECT id, track_id, event_kind, seconds, occurred_at
         FROM play_events
         WHERE user_id = ",
    );
    builder.push_bind(user_id);

    if let Some(cursor) = cursor {
        builder
            .push(" AND (occurred_at < ")
            .push_bind(cursor.key.as_str())
            .push(" OR (occurred_at = ")
            .push_bind(cursor.key.as_str())
            .push(" AND id < ")
            .push_bind(cursor.id.as_str())
            .push("))");
    }

    builder
        .push(" ORDER BY occurred_at DESC, id DESC LIMIT ")
        .push_bind(fetch);

    let rows = builder
        .build()
        .fetch_all(pool)
        .await
        .with_context(|| format!("falha ao listar o histórico do usuário '{user_id}'"))?;

    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        items.push(HistoryRow {
            id: row.try_get("id").context("coluna `id` do histórico")?,
            track_id: row
                .try_get("track_id")
                .context("coluna `track_id` do histórico")?,
            event_kind: row
                .try_get("event_kind")
                .context("coluna `event_kind` do histórico")?,
            seconds: row
                .try_get("seconds")
                .context("coluna `seconds` do histórico")?,
            occurred_at: row
                .try_get("occurred_at")
                .context("coluna `occurred_at` do histórico")?,
        });
    }

    let has_more = items.len() > page_size;
    items.truncate(page_size);

    // O cursor vem da última linha incluída, nunca da linha extra descartada.
    let next_cursor = if has_more {
        items
            .last()
            .map(|row| Cursor::encode(&row.occurred_at, &row.id))
    } else {
        None
    };

    Ok((items, next_cursor))
}

/// Apaga todo o histórico do usuário. Devolve quantos eventos foram removidos.
pub async fn clear_history(pool: &SqlitePool, user_id: &str) -> Result<u64> {
    let result = sqlx::query("DELETE FROM play_events WHERE user_id = ?1")
        .bind(user_id)
        .execute(pool)
        .await
        .with_context(|| format!("falha ao apagar o histórico do usuário '{user_id}'"))?;

    Ok(result.rows_affected())
}

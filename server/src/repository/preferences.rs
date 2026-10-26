//! Preferências de interface e privacidade do usuário (M2e).
//!
//! Regras:
//! * sempre uma linha por usuário (`user_id` é a PK); o backend cria a linha com
//!   os defaults na primeira leitura, então `GET` nunca falha por ausência;
//! * mutações usam `ON CONFLICT(user_id) DO UPDATE` (upsert) e atualizam
//!   `updated_at` mesmo quando o corpo não traz nenhum campo;
//! * `user_id` vem sempre do `CurrentUser`, nunca do corpo da requisição;
//! * SQL parametrizado.

use anyhow::{Context, Result};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

/// Expressão SQLite de "agora" no formato ISO-8601 UTC (mesmo padrão das migrations).
const NOW_SQL: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

/// Tema padrão de uma conta nova.
pub const DEFAULT_THEME: &str = "system";
/// Idioma padrão de uma conta nova.
pub const DEFAULT_LANGUAGE: &str = "pt-BR";

/// Preferências de interface persistidas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferencesRecord {
    pub theme: String,
    pub language: String,
}

/// Preferências de privacidade persistidas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyRecord {
    pub allow_cloud_history: bool,
    /// `None` = histórico sem expiração.
    pub history_retention_days: Option<i64>,
}

/// Carrega as preferências, criando a linha com os defaults quando necessário.
pub async fn ensure_preferences(pool: &SqlitePool, user_id: &str) -> Result<PreferencesRecord> {
    sqlx::query(
        "INSERT INTO user_preferences (user_id, theme, language)
         VALUES (?1, ?2, ?3)
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(DEFAULT_THEME)
    .bind(DEFAULT_LANGUAGE)
    .execute(pool)
    .await
    .with_context(|| format!("falha ao garantir as preferências do usuário '{user_id}'"))?;

    load_preferences(pool, user_id)
        .await?
        .with_context(|| format!("preferências ausentes para o usuário '{user_id}'"))
}

/// Aplica um upsert parcial nas preferências e devolve o estado final.
pub async fn update_preferences(
    pool: &SqlitePool,
    user_id: &str,
    theme: Option<&str>,
    language: Option<&str>,
) -> Result<PreferencesRecord> {
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        "INSERT INTO user_preferences (user_id, theme, language, updated_at)
         VALUES (",
    );
    builder
        .push_bind(user_id)
        .push(", ")
        .push_bind(theme.unwrap_or(DEFAULT_THEME))
        .push(", ")
        .push_bind(language.unwrap_or(DEFAULT_LANGUAGE))
        .push(format!(", {NOW_SQL})"))
        .push(" ON CONFLICT (user_id) DO UPDATE SET updated_at = ")
        .push(NOW_SQL);

    if let Some(theme) = theme {
        builder.push(", theme = ").push_bind(theme);
    }

    if let Some(language) = language {
        builder.push(", language = ").push_bind(language);
    }

    builder
        .build()
        .execute(pool)
        .await
        .with_context(|| format!("falha ao atualizar as preferências do usuário '{user_id}'"))?;

    load_preferences(pool, user_id)
        .await?
        .with_context(|| format!("preferências ausentes para o usuário '{user_id}'"))
}

/// Carrega as preferências de privacidade, criando a linha default se preciso.
pub async fn ensure_privacy(pool: &SqlitePool, user_id: &str) -> Result<PrivacyRecord> {
    sqlx::query(
        "INSERT INTO user_privacy (user_id, allow_cloud_history, history_retention_days)
         VALUES (?1, 1, NULL)
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .with_context(|| format!("falha ao garantir a privacidade do usuário '{user_id}'"))?;

    load_privacy(pool, user_id)
        .await?
        .with_context(|| format!("privacidade ausente para o usuário '{user_id}'"))
}

/// Aplica um upsert parcial na privacidade e devolve o estado final.
///
/// `history_retention_days` distingue "campo omitido" (`None`) de "limpar a
/// retenção" (`Some(None)`).
pub async fn update_privacy(
    pool: &SqlitePool,
    user_id: &str,
    allow_cloud_history: Option<bool>,
    history_retention_days: Option<Option<i64>>,
) -> Result<PrivacyRecord> {
    let allow = allow_cloud_history.unwrap_or(true);

    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        "INSERT INTO user_privacy (user_id, allow_cloud_history, history_retention_days, updated_at)
         VALUES (",
    );
    builder
        .push_bind(user_id)
        .push(", ")
        .push_bind(if allow { 1_i64 } else { 0_i64 })
        .push(", ")
        .push_bind(history_retention_days.flatten())
        .push(format!(", {NOW_SQL})"))
        .push(" ON CONFLICT (user_id) DO UPDATE SET updated_at = ")
        .push(NOW_SQL);

    if let Some(allow) = allow_cloud_history {
        builder
            .push(", allow_cloud_history = ")
            .push_bind(if allow { 1_i64 } else { 0_i64 });
    }

    if let Some(retention) = history_retention_days {
        builder
            .push(", history_retention_days = ")
            .push_bind(retention);
    }

    builder
        .build()
        .execute(pool)
        .await
        .with_context(|| format!("falha ao atualizar a privacidade do usuário '{user_id}'"))?;

    load_privacy(pool, user_id)
        .await?
        .with_context(|| format!("privacidade ausente para o usuário '{user_id}'"))
}

// ---------------------------------------------------------------------------
// Helpers privados
// ---------------------------------------------------------------------------

async fn load_preferences(pool: &SqlitePool, user_id: &str) -> Result<Option<PreferencesRecord>> {
    let row = sqlx::query("SELECT theme, language FROM user_preferences WHERE user_id = ?1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .with_context(|| format!("falha ao carregar as preferências do usuário '{user_id}'"))?;

    row.map(|row| {
        Ok(PreferencesRecord {
            theme: row
                .try_get("theme")
                .context("coluna `theme` das preferências")?,
            language: row
                .try_get("language")
                .context("coluna `language` das preferências")?,
        })
    })
    .transpose()
}

async fn load_privacy(pool: &SqlitePool, user_id: &str) -> Result<Option<PrivacyRecord>> {
    let row = sqlx::query(
        "SELECT allow_cloud_history, history_retention_days
         FROM user_privacy WHERE user_id = ?1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .with_context(|| format!("falha ao carregar a privacidade do usuário '{user_id}'"))?;

    row.map(|row| {
        let allow_cloud_history: i64 = row
            .try_get("allow_cloud_history")
            .context("coluna `allow_cloud_history` da privacidade")?;

        Ok(PrivacyRecord {
            allow_cloud_history: allow_cloud_history != 0,
            history_retention_days: row
                .try_get("history_retention_days")
                .context("coluna `history_retention_days` da privacidade")?,
        })
    })
    .transpose()
}

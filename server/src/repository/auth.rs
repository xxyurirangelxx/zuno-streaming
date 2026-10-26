//! Persistência de usuários, sessões, setup, convites e auditoria (M2a).
//!
//! Datas são calculadas pelo SQLite (`strftime`) para não introduzir crate de
//! calendário: as funções recebem modificadores como `"+30 days"`.

use anyhow::{Context, Result};
use sqlx::{Row, SqlitePool};

use crate::auth::Role;

/// Usuário persistido (inclui hash da senha — nunca expor em JSON).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: Role,
    pub disabled: bool,
}

/// Sessão válida com os dados do usuário (resultado do join).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub session_id: String,
    pub user_id: String,
    pub username: String,
    pub role: Role,
    pub disabled: bool,
}

/// Convite persistido (sem o token em claro).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationRecord {
    pub id: String,
    pub role: Role,
    pub expires_at: String,
    pub redeemed_at: Option<String>,
    pub revoked_at: Option<String>,
}

/// Conta usuários ativos + desabilitados (usado para decidir o setup inicial).
pub async fn count_users(pool: &SqlitePool) -> Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .context("falha ao contar usuários")
}

/// Insere um usuário; erro quando o nome normalizado já existe.
pub async fn insert_user(
    pool: &SqlitePool,
    id: &str,
    username: &str,
    normalized_username: &str,
    password_hash: &str,
    role: Role,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO users (id, username, normalized_username, password_hash, role)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(id)
    .bind(username)
    .bind(normalized_username)
    .bind(password_hash)
    .bind(role.as_str())
    .execute(pool)
    .await
    .context("falha ao criar usuário")?;

    Ok(())
}

/// Busca um usuário pelo nome normalizado.
pub async fn find_user_by_username(
    pool: &SqlitePool,
    normalized_username: &str,
) -> Result<Option<UserRecord>> {
    let row = sqlx::query(
        "SELECT id, username, password_hash, role, disabled_at
         FROM users WHERE normalized_username = ?1",
    )
    .bind(normalized_username)
    .fetch_optional(pool)
    .await
    .context("falha ao consultar usuário")?;

    row.map(map_user).transpose()
}

/// Busca um usuário pelo id.
pub async fn find_user_by_id(pool: &SqlitePool, user_id: &str) -> Result<Option<UserRecord>> {
    let row = sqlx::query(
        "SELECT id, username, password_hash, role, disabled_at FROM users WHERE id = ?1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("falha ao consultar usuário")?;

    row.map(map_user).transpose()
}

fn map_user(row: sqlx::sqlite::SqliteRow) -> Result<UserRecord> {
    let role: String = row.try_get("role")?;
    let disabled_at: Option<String> = row.try_get("disabled_at")?;

    Ok(UserRecord {
        id: row.try_get("id")?,
        username: row.try_get("username")?,
        password_hash: row.try_get("password_hash")?,
        role: Role::parse(&role).unwrap_or(Role::User),
        disabled: disabled_at.is_some(),
    })
}

/// Cria uma sessão com validade relativa (ex.: `"+30 days"`).
pub async fn create_session(
    pool: &SqlitePool,
    id: &str,
    user_id: &str,
    token_hash: &str,
    ttl_modifier: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, expires_at)
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?4))",
    )
    .bind(id)
    .bind(user_id)
    .bind(token_hash)
    .bind(ttl_modifier)
    .execute(pool)
    .await
    .context("falha ao criar sessão")?;

    Ok(())
}

/// Busca uma sessão válida (não expirada, não revogada) pelo hash do token.
pub async fn find_valid_session(
    pool: &SqlitePool,
    token_hash: &str,
) -> Result<Option<SessionRecord>> {
    let row = sqlx::query(
        "SELECT s.id AS session_id, u.id AS user_id, u.username, u.role, u.disabled_at
         FROM sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ?1
           AND s.revoked_at IS NULL
           AND s.expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .context("falha ao consultar sessão")?;

    row.map(|row| {
        let role: String = row.try_get("role")?;
        let disabled_at: Option<String> = row.try_get("disabled_at")?;
        Ok(SessionRecord {
            session_id: row.try_get("session_id")?,
            user_id: row.try_get("user_id")?,
            username: row.try_get("username")?,
            role: Role::parse(&role).unwrap_or(Role::User),
            disabled: disabled_at.is_some(),
        })
    })
    .transpose()
}

/// Marca a sessão como vista agora (best effort, não bloqueia a requisição).
pub async fn touch_session(pool: &SqlitePool, session_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE sessions SET last_seen_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .context("falha ao atualizar sessão")?;
    Ok(())
}

/// Revoga uma sessão (logout ou rotação).
pub async fn revoke_session(pool: &SqlitePool, session_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE sessions SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .context("falha ao revogar sessão")?;
    Ok(())
}

/// Revoga todas as sessões de um usuário (troca de senha, desabilitação).
pub async fn revoke_user_sessions(pool: &SqlitePool, user_id: &str) -> Result<u64> {
    let result = sqlx::query(
        "UPDATE sessions SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE user_id = ?1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .context("falha ao revogar sessões do usuário")?;

    Ok(result.rows_affected())
}

/// Cria o token de setup inicial com validade relativa.
pub async fn insert_setup_token(
    pool: &SqlitePool,
    id: &str,
    token_hash: &str,
    ttl_modifier: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO setup_tokens (id, token_hash, expires_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?3))",
    )
    .bind(id)
    .bind(token_hash)
    .bind(ttl_modifier)
    .execute(pool)
    .await
    .context("falha ao criar código de setup")?;

    Ok(())
}

/// Verifica se existe um código de setup válido (não usado, não expirado).
pub async fn has_valid_setup_token(pool: &SqlitePool) -> Result<bool> {
    let found: Option<String> = sqlx::query_scalar(
        "SELECT id FROM setup_tokens
         WHERE used_at IS NULL
           AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .context("falha ao consultar código de setup")?;

    Ok(found.is_some())
}

/// Consome um código de setup válido (uso único). Retorna `false` se inválido.
pub async fn consume_setup_token(pool: &SqlitePool, token_hash: &str) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE setup_tokens SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE token_hash = ?1
           AND used_at IS NULL
           AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(token_hash)
    .execute(pool)
    .await
    .context("falha ao consumir código de setup")?;

    Ok(result.rows_affected() == 1)
}

/// Cria um convite.
pub async fn insert_invitation(
    pool: &SqlitePool,
    id: &str,
    token_hash: &str,
    role: Role,
    created_by: Option<&str>,
    ttl_modifier: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO invitations (id, token_hash, role, created_by, expires_at)
         VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?5))",
    )
    .bind(id)
    .bind(token_hash)
    .bind(role.as_str())
    .bind(created_by)
    .bind(ttl_modifier)
    .execute(pool)
    .await
    .context("falha ao criar convite")?;

    Ok(())
}

/// Busca um convite válido (não usado, não revogado, não expirado).
pub async fn find_valid_invitation(
    pool: &SqlitePool,
    token_hash: &str,
) -> Result<Option<InvitationRecord>> {
    let row = sqlx::query(
        "SELECT id, role, expires_at, redeemed_at, revoked_at
         FROM invitations
         WHERE token_hash = ?1
           AND redeemed_at IS NULL
           AND revoked_at IS NULL
           AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .context("falha ao consultar convite")?;

    row.map(|row| {
        let role: String = row.try_get("role")?;
        Ok(InvitationRecord {
            id: row.try_get("id")?,
            role: Role::parse(&role).unwrap_or(Role::User),
            expires_at: row.try_get("expires_at")?,
            redeemed_at: row.try_get("redeemed_at")?,
            revoked_at: row.try_get("revoked_at")?,
        })
    })
    .transpose()
}

/// Marca o convite como usado (uso único).
pub async fn consume_invitation(pool: &SqlitePool, invitation_id: &str) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE invitations SET redeemed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1 AND redeemed_at IS NULL AND revoked_at IS NULL",
    )
    .bind(invitation_id)
    .execute(pool)
    .await
    .context("falha ao consumir convite")?;

    Ok(result.rows_affected() == 1)
}

/// Lista convites pendentes (admin).
pub async fn list_invitations(pool: &SqlitePool) -> Result<Vec<InvitationRecord>> {
    let rows = sqlx::query(
        "SELECT id, role, expires_at, redeemed_at, revoked_at
         FROM invitations ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(pool)
    .await
    .context("falha ao listar convites")?;

    rows.into_iter()
        .map(|row| {
            let role: String = row.try_get("role")?;
            Ok(InvitationRecord {
                id: row.try_get("id")?,
                role: Role::parse(&role).unwrap_or(Role::User),
                expires_at: row.try_get("expires_at")?,
                redeemed_at: row.try_get("redeemed_at")?,
                revoked_at: row.try_get("revoked_at")?,
            })
        })
        .collect()
}

/// Revoga um convite pendente.
pub async fn revoke_invitation(pool: &SqlitePool, invitation_id: &str) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE invitations SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1 AND redeemed_at IS NULL AND revoked_at IS NULL",
    )
    .bind(invitation_id)
    .execute(pool)
    .await
    .context("falha ao revogar convite")?;

    Ok(result.rows_affected() == 1)
}

/// Registra um evento de auditoria administrativa.
pub async fn record_audit(
    pool: &SqlitePool,
    actor_id: Option<&str>,
    action: &str,
    target: Option<&str>,
    detail: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_events (actor_id, action, target, detail) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(actor_id)
    .bind(action)
    .bind(target)
    .bind(detail)
    .execute(pool)
    .await
    .context("falha ao registrar auditoria")?;

    Ok(())
}

/// Define um novo hash de senha e revoga as sessões existentes.
pub async fn update_password(pool: &SqlitePool, user_id: &str, password_hash: &str) -> Result<()> {
    sqlx::query("UPDATE users SET password_hash = ?1 WHERE id = ?2")
        .bind(password_hash)
        .bind(user_id)
        .execute(pool)
        .await
        .context("falha ao atualizar senha")?;

    revoke_user_sessions(pool, user_id).await?;
    Ok(())
}

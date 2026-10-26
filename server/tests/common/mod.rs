//! Helpers compartilhados pelos testes de integração HTTP.
//!
//! Cada arquivo de teste usa apenas um subconjunto destes helpers, por isso o
//! `allow(dead_code)` no topo do módulo.
#![allow(dead_code)]

use sqlx::SqlitePool;
use uuid::Uuid;
use zuno_server::auth::{Role, generate_token, hash_password, hash_token};
use zuno_server::domain::normalize_key;
use zuno_server::repository::auth;

/// Senha usada pelos usuários criados pelos helpers.
pub const TEST_PASSWORD: &str = "senha-de-teste";

/// Cria um administrador + sessão e devolve o cabeçalho `Cookie` pronto.
pub async fn admin_cookie(pool: &SqlitePool) -> String {
    user_cookie(pool, "admin", Role::Admin).await
}

/// Cria um usuário com o papel informado + sessão e devolve o `Cookie`.
pub async fn user_cookie(pool: &SqlitePool, username: &str, role: Role) -> String {
    let id = Uuid::now_v7().to_string();
    let password_hash = hash_password(TEST_PASSWORD).expect("hash da senha de teste");

    auth::insert_user(
        pool,
        &id,
        username,
        &normalize_key(username),
        &password_hash,
        role,
    )
    .await
    .expect("inserir usuário de teste");

    let token = generate_token();
    auth::create_session(
        pool,
        &Uuid::now_v7().to_string(),
        &id,
        &hash_token(&token),
        "+30 days",
    )
    .await
    .expect("criar sessão de teste");

    format!("zuno_session={token}")
}

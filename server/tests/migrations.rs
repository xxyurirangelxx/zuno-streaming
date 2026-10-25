use tempfile::TempDir;
use zuno_server::{Config, Database};

struct TestContext {
    _dir: TempDir,
    database_url: String,
    db: Database,
}

async fn test_context() -> TestContext {
    let dir = tempfile::tempdir().expect("diretório temporário");
    let db_path = dir.path().join("zuno.db");
    let database_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let config = Config {
        bind_addr: "127.0.0.1:0".parse().expect("bind de teste"),
        database_url: database_url.clone(),
        music_dir: dir.path().join("music"),
        db_max_connections: 2,
    };

    let db = Database::connect(&config)
        .await
        .expect("banco de teste deve conectar");

    TestContext {
        _dir: dir,
        database_url,
        db,
    }
}

#[tokio::test]
async fn migrations_create_baseline_metadata() {
    let context = test_context().await;

    let baseline: String =
        sqlx::query_scalar("SELECT value FROM zuno_meta WHERE key = 'schema_baseline'")
            .fetch_one(context.db.pool())
            .await
            .expect("schema_baseline deve existir");
    assert_eq!(baseline, "m0");

    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(context.db.pool())
        .await
        .expect("histórico de migrations");
    assert_eq!(applied, 1, "deve haver exatamente uma migration aplicada");
}

#[tokio::test]
async fn sqlite_pragmas_are_enabled() {
    let context = test_context().await;

    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(context.db.pool())
        .await
        .expect("journal_mode");
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(context.db.pool())
        .await
        .expect("foreign_keys");
    assert_eq!(foreign_keys, 1);
}

#[tokio::test]
async fn migrations_are_idempotent_on_reconnect() {
    let context = test_context().await;

    let config = Config {
        bind_addr: "127.0.0.1:0".parse().expect("bind de teste"),
        database_url: context.database_url.clone(),
        music_dir: context._dir.path().join("music"),
        db_max_connections: 2,
    };
    let reconnected = Database::connect(&config)
        .await
        .expect("reconexão deve ser bem-sucedida");

    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(reconnected.pool())
        .await
        .expect("histórico de migrations");
    assert_eq!(applied, 1, "migrations não devem reaplicar");
}

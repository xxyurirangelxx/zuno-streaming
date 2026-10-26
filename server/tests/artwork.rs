use std::fs;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;
use zuno_server::{AppState, Config, Database, LibraryScanner, ScanOptions, build_router};

mod common;

struct TestEnv {
    _dir: TempDir,
    database: Database,
    config: Config,
    cookie: String,
    artwork_id: String,
    derived_dir: std::path::PathBuf,
}

async fn setup() -> TestEnv {
    let dir = tempfile::tempdir().expect("diretório temporário");
    let music_dir = dir.path().join("music");
    fs::create_dir_all(&music_dir).expect("criar biblioteca");

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/audio/tone-a.opus");
    fs::copy(&source, music_dir.join("tone-a.opus")).expect("copiar fixture");

    let artwork_dir = dir.path().join("artwork");
    let config = Config {
        bind_addr: "127.0.0.1:0".parse().expect("bind"),
        database_url: format!(
            "sqlite://{}",
            dir.path()
                .join("zuno.db")
                .to_string_lossy()
                .replace('\\', "/")
        ),
        music_dir,
        artwork_dir: artwork_dir.clone(),
        staging_dir: dir.path().join("staging"),
        max_audio_file_bytes: 10 * 1024 * 1024,
        scan_on_startup: false,
        cookie_secure: false,
        db_max_connections: 2,
    };

    let database = Database::connect(&config)
        .await
        .expect("banco de teste deve conectar");
    LibraryScanner::new(&config, ScanOptions::from_config(&config))
        .expect("scanner")
        .scan(&database)
        .await
        .expect("scan");

    let artwork_id: String = sqlx::query_scalar("SELECT id FROM artworks LIMIT 1")
        .fetch_one(database.pool())
        .await
        .expect("capa indexada");
    let cookie = common::admin_cookie(database.pool()).await;

    TestEnv {
        _dir: dir,
        database,
        config,
        cookie,
        artwork_id,
        derived_dir: artwork_dir.join("derived"),
    }
}

fn app(env: &TestEnv) -> axum::Router {
    build_router(AppState::new(
        env.database.pool().clone(),
        env.config.clone(),
        "0.1.0",
    ))
}

async fn get(env: &TestEnv, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = app(env)
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(header::COOKIE, env.cookie.as_str())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("resposta");

    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("corpo")
        .to_bytes()
        .to_vec();

    (status, headers, bytes)
}

#[tokio::test]
async fn full_artwork_is_served_with_original_mime() {
    let env = setup().await;
    let uri = format!("/api/v1/artworks/{}", env.artwork_id);

    let (status, headers, body) = get(&env, &uri).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert!(!body.is_empty());
}

#[tokio::test]
async fn derived_artwork_is_resized_and_cached() {
    let env = setup().await;

    let uri = format!("/api/v1/artworks/{}?size=thumb", env.artwork_id);
    let (status, headers, body) = get(&env, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
    assert!(!body.is_empty());

    let cached = env.derived_dir.join(format!("{}-320.jpg", env.artwork_id));
    assert!(cached.exists(), "variante deve ser gravada no cache");
    assert_eq!(
        fs::read(&cached).expect("ler cache"),
        body,
        "segunda leitura deve vir do cache"
    );

    // Requisição repetida continua válida e idêntica.
    let (status, _, again) = get(&env, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again, body);
}

#[tokio::test]
async fn invalid_size_and_unknown_artwork_are_handled() {
    let env = setup().await;

    let (status, _, body) = get(
        &env,
        &format!("/api/v1/artworks/{}?size=gigante", env.artwork_id),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let json: Value = serde_json::from_slice(&body).expect("JSON");
    assert_eq!(json["code"], "invalid_query");

    let (status, _, body) = get(&env, "/api/v1/artworks/nao-existe").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: Value = serde_json::from_slice(&body).expect("JSON");
    assert_eq!(json["code"], "not_found");
}

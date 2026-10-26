use std::fs;
use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;
use zuno_server::{AppState, Config, Database, LibraryScanner, ScanOptions, build_router};

struct TestEnv {
    _dir: TempDir,
    database: Database,
    config: Config,
    track_id: String,
    audio_path: PathBuf,
    audio_size: u64,
    etag: String,
}

async fn setup() -> TestEnv {
    let dir = tempfile::tempdir().expect("diretório temporário");
    let music_dir = dir.path().join("music");
    fs::create_dir_all(&music_dir).expect("criar biblioteca");

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/audio/tone-a.opus");
    fs::copy(&source, music_dir.join("tone-a.opus")).expect("copiar fixture");

    let config = Config {
        bind_addr: "127.0.0.1:0".parse().expect("bind"),
        database_url: format!(
            "sqlite://{}",
            dir.path()
                .join("zuno.db")
                .to_string_lossy()
                .replace('\\', "/")
        ),
        music_dir: music_dir.clone(),
        artwork_dir: dir.path().join("artwork"),
        staging_dir: dir.path().join("staging"),
        max_audio_file_bytes: 10 * 1024 * 1024,
        scan_on_startup: false,
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

    let track_id: String = sqlx::query_scalar("SELECT id FROM tracks WHERE title = 'Tone A'")
        .fetch_one(database.pool())
        .await
        .expect("faixa indexada");
    let sha256: String = sqlx::query_scalar("SELECT sha256 FROM audio_files LIMIT 1")
        .fetch_one(database.pool())
        .await
        .expect("arquivo indexado");

    let audio_path = music_dir.join("tone-a.opus");
    let audio_size = fs::metadata(&audio_path).expect("metadados").len();

    TestEnv {
        _dir: dir,
        database,
        config,
        track_id,
        audio_path,
        audio_size,
        etag: format!("\"{sha256}\""),
    }
}

fn app(env: &TestEnv) -> axum::Router {
    build_router(AppState::new(
        env.database.pool().clone(),
        env.config.clone(),
        "0.1.0",
    ))
}

async fn request(
    env: &TestEnv,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }

    let response = app(env)
        .oneshot(builder.body(Body::empty()).expect("request"))
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
async fn full_stream_returns_audio_headers_and_bytes() {
    let env = setup().await;
    let uri = format!("/api/v1/tracks/{}/stream", env.track_id);

    let (status, headers, body) = request(&env, Method::GET, &uri, &[]).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "audio/ogg");
    assert_eq!(headers[header::ACCEPT_RANGES], "bytes");
    assert_eq!(headers[header::ETAG], env.etag.as_str());
    assert_eq!(body.len() as u64, env.audio_size);

    let expected = fs::read(&env.audio_path).expect("ler arquivo");
    assert_eq!(body, expected);
}

#[tokio::test]
async fn head_stream_has_headers_and_empty_body() {
    let env = setup().await;
    let uri = format!("/api/v1/tracks/{}/stream", env.track_id);

    let (status, headers, body) = request(&env, Method::HEAD, &uri, &[]).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_LENGTH],
        env.audio_size.to_string().as_str()
    );
    assert!(body.is_empty());
}

#[tokio::test]
async fn range_requests_return_206_with_content_range() {
    let env = setup().await;
    let uri = format!("/api/v1/tracks/{}/stream", env.track_id);
    let expected = fs::read(&env.audio_path).expect("ler arquivo");

    let (status, headers, body) =
        request(&env, Method::GET, &uri, &[("range", "bytes=0-99")]).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        headers[header::CONTENT_RANGE],
        format!("bytes 0-99/{}", env.audio_size).as_str()
    );
    assert_eq!(body.len(), 100);
    assert_eq!(body, expected[..100]);

    let (status, headers, body) =
        request(&env, Method::GET, &uri, &[("range", "bytes=100-")]).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        headers[header::CONTENT_RANGE],
        format!("bytes 100-{}/{}", env.audio_size - 1, env.audio_size).as_str()
    );
    assert_eq!(body, expected[100..]);

    let (status, _, body) = request(&env, Method::GET, &uri, &[("range", "bytes=-200")]).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body.len(), 200);
    assert_eq!(body, expected[expected.len() - 200..]);
}

#[tokio::test]
async fn unsatisfiable_range_returns_416_with_json_error() {
    let env = setup().await;
    let uri = format!("/api/v1/tracks/{}/stream", env.track_id);

    let range_value = format!("bytes={}-", env.audio_size);
    let (status, headers, body) =
        request(&env, Method::GET, &uri, &[("range", range_value.as_str())]).await;

    assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        headers[header::CONTENT_RANGE],
        format!("bytes */{}", env.audio_size).as_str()
    );

    let json: Value = serde_json::from_slice(&body).expect("JSON de erro");
    assert_eq!(json["code"], "range_not_satisfiable");
}

#[tokio::test]
async fn if_range_controls_partial_serving() {
    let env = setup().await;
    let uri = format!("/api/v1/tracks/{}/stream", env.track_id);
    let expected = fs::read(&env.audio_path).expect("ler arquivo");

    // ETag correspondente: aplica o Range.
    let (status, _, body) = request(
        &env,
        Method::GET,
        &uri,
        &[("range", "bytes=0-49"), ("if-range", env.etag.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body.len(), 50);

    // ETag divergente: devolve a representação completa.
    let (status, headers, body) = request(
        &env,
        Method::GET,
        &uri,
        &[("range", "bytes=0-49"), ("if-range", "\"outro-etag\"")],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_LENGTH],
        env.audio_size.to_string().as_str()
    );
    assert_eq!(body, expected);
}

#[tokio::test]
async fn unknown_track_and_missing_file_return_404() {
    let env = setup().await;

    let (status, _, body) = request(
        &env,
        Method::GET,
        "/api/v1/tracks/00000000-0000-7000-8000-000000000000/stream",
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: Value = serde_json::from_slice(&body).expect("JSON");
    assert_eq!(json["code"], "not_found");

    // Arquivo indexado, mas removido do disco: nunca servimos caminho inválido.
    fs::remove_file(&env.audio_path).expect("remover arquivo");
    let uri = format!("/api/v1/tracks/{}/stream", env.track_id);
    let (status, _, _) = request(&env, Method::GET, &uri, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

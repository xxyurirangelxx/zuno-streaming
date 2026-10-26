use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;
use zuno_server::{AppState, Config, Database, build_router};

struct TestContext {
    _dir: TempDir,
    db: Database,
}

async fn test_context() -> TestContext {
    let dir = tempfile::tempdir().expect("diretório temporário");
    let db_path = dir.path().join("zuno.db");
    let database_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let config = Config {
        bind_addr: "127.0.0.1:0".parse().expect("bind de teste"),
        database_url,
        music_dir: dir.path().join("music"),
        artwork_dir: dir.path().join("artwork"),
        max_audio_file_bytes: 10 * 1024 * 1024,
        scan_on_startup: false,
        db_max_connections: 2,
    };

    let db = Database::connect(&config)
        .await
        .expect("banco de teste deve conectar");

    TestContext { _dir: dir, db }
}

fn app_for(db: &Database) -> axum::Router {
    build_router(AppState::new(db.pool().clone(), env!("CARGO_PKG_VERSION")))
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("corpo da resposta")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("JSON válido")
}

#[tokio::test]
async fn health_returns_ok_contract() {
    let context = test_context().await;
    let response = app_for(&context.db)
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("resposta");

    assert_eq!(response.status(), StatusCode::OK);

    let json = body_json(response).await;
    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "zuno-server");
    assert_eq!(json["version"], "0.1.0");
    assert_eq!(json["database"]["status"], "ok");
    assert!(
        json["uptime_seconds"].is_u64(),
        "uptime_seconds deve ser numérico"
    );
}

#[tokio::test]
async fn unknown_route_returns_json_not_found() {
    let context = test_context().await;
    let response = app_for(&context.db)
        .oneshot(
            Request::builder()
                .uri("/rota-inexistente")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("resposta");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let json = body_json(response).await;
    assert_eq!(json["code"], "not_found");
    assert!(json["message"].is_string());
}

#[tokio::test]
async fn health_returns_degraded_when_database_is_closed() {
    let context = test_context().await;
    let app = app_for(&context.db);

    context.db.pool().close().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("resposta");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let json = body_json(response).await;
    assert_eq!(json["status"], "degraded");
    assert_eq!(json["database"]["status"], "error");
}

//! Testes de integração HTTP de preferências e privacidade (M2e).
//!
//! Tudo roda em `TempDir`: banco SQLite migrado na hora, sem rede.

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;
use zuno_server::auth::Role;
use zuno_server::{AppState, Config, Database, build_router};

mod common;

struct TestContext {
    _dir: TempDir,
    db: Database,
    config: Config,
    cookie: String,
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
        staging_dir: dir.path().join("staging"),
        max_audio_file_bytes: 10 * 1024 * 1024,
        scan_on_startup: false,
        cookie_secure: false,
        db_max_connections: 2,
    };

    let db = Database::connect(&config)
        .await
        .expect("banco de teste deve conectar");
    let cookie = common::admin_cookie(db.pool()).await;

    TestContext {
        _dir: dir,
        db,
        config,
        cookie,
    }
}

fn router_for(context: &TestContext) -> Router {
    build_router(AppState::new(
        context.db.pool().clone(),
        context.config.clone(),
        "0.1.0",
    ))
}

async fn call(
    router: &Router,
    cookie: Option<&str>,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);

    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }

    let body = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };

    let response = router
        .clone()
        .oneshot(builder.body(body).expect("request"))
        .await
        .expect("resposta");

    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("corpo da resposta")
        .to_bytes();

    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("JSON válido")
    };

    (status, json)
}

async fn get(router: &Router, cookie: &str, uri: &str) -> (StatusCode, Value) {
    call(router, Some(cookie), Method::GET, uri, None).await
}

async fn put(router: &Router, cookie: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    call(router, Some(cookie), Method::PUT, uri, Some(body)).await
}

#[tokio::test]
async fn defaults_are_created_on_first_read() {
    let context = test_context().await;
    let router = router_for(&context);
    let cookie = context.cookie.clone();

    let (status, body) = get(&router, &cookie, "/api/v1/me/preferences").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["theme"], "system");
    assert_eq!(body["language"], "pt-BR");

    let (status, body) = get(&router, &cookie, "/api/v1/me/privacy").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["allow_cloud_history"], true);
    assert!(body["history_retention_days"].is_null());

    // A leitura é idempotente: não cria duplicatas nem muda os valores.
    let (status, body) = get(&router, &cookie, "/api/v1/me/preferences").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["theme"], "system");
    assert_eq!(body["language"], "pt-BR");
}

#[tokio::test]
async fn preferences_update_and_validation() {
    let context = test_context().await;
    let router = router_for(&context);
    let cookie = context.cookie.clone();

    let (status, body) = put(
        &router,
        &cookie,
        "/api/v1/me/preferences",
        json!({ "theme": "dark", "language": "en" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["theme"], "dark");
    assert_eq!(body["language"], "en");

    // Atualização parcial mantém o outro campo.
    let (status, body) = put(
        &router,
        &cookie,
        "/api/v1/me/preferences",
        json!({ "theme": "light" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["theme"], "light");
    assert_eq!(body["language"], "en");

    let (status, body) = put(
        &router,
        &cookie,
        "/api/v1/me/preferences",
        json!({ "theme": "neon" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid_query");

    for language in ["p", "pt-BR-extra-longo", "pt_BR!", ""] {
        let (status, body) = put(
            &router,
            &cookie,
            "/api/v1/me/preferences",
            json!({ "language": language }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "language {language}");
        assert_eq!(body["code"], "invalid_query", "language {language}");
    }

    // Nada foi alterado pelas tentativas inválidas.
    let (status, body) = get(&router, &cookie, "/api/v1/me/preferences").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["theme"], "light");
    assert_eq!(body["language"], "en");
}

#[tokio::test]
async fn preferences_are_isolated_per_user() {
    let context = test_context().await;
    let router = router_for(&context);
    let alice = context.cookie.clone();
    let bob = common::user_cookie(context.db.pool(), "bob", Role::User).await;

    let (status, _) = put(
        &router,
        &alice,
        "/api/v1/me/preferences",
        json!({ "theme": "dark", "language": "en" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get(&router, &alice, "/api/v1/me/preferences").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["theme"], "dark");

    // Bob nunca vê as preferências de Alice.
    let (status, body) = get(&router, &bob, "/api/v1/me/preferences").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["theme"], "system");
    assert_eq!(body["language"], "pt-BR");

    // E as mutações de Bob não afetam Alice.
    let (status, _) = put(
        &router,
        &bob,
        "/api/v1/me/preferences",
        json!({ "theme": "light" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get(&router, &alice, "/api/v1/me/preferences").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["theme"], "dark");
}

#[tokio::test]
async fn privacy_update_validation_and_isolation() {
    let context = test_context().await;
    let router = router_for(&context);
    let alice = context.cookie.clone();
    let bob = common::user_cookie(context.db.pool(), "bob", Role::User).await;

    let (status, body) = put(
        &router,
        &alice,
        "/api/v1/me/privacy",
        json!({ "allow_cloud_history": false, "history_retention_days": 30 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["allow_cloud_history"], false);
    assert_eq!(body["history_retention_days"], 30);

    // Atualização parcial mantém a retenção.
    let (status, body) = put(
        &router,
        &alice,
        "/api/v1/me/privacy",
        json!({ "allow_cloud_history": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["allow_cloud_history"], true);
    assert_eq!(body["history_retention_days"], 30);

    // `null` limpa a retenção (histórico eterno).
    let (status, body) = put(
        &router,
        &alice,
        "/api/v1/me/privacy",
        json!({ "history_retention_days": null }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["history_retention_days"].is_null());

    for days in [0, -1, 3651, 100_000] {
        let (status, body) = put(
            &router,
            &alice,
            "/api/v1/me/privacy",
            json!({ "history_retention_days": days }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "retention {days}");
        assert_eq!(body["code"], "invalid_query", "retention {days}");
    }

    // Limites aceitos.
    for days in [1, 3650] {
        let (status, body) = put(
            &router,
            &alice,
            "/api/v1/me/privacy",
            json!({ "history_retention_days": days }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "retention {days}");
        assert_eq!(body["history_retention_days"], days);
    }

    // Isolamento: Bob mantém os defaults.
    let (status, body) = get(&router, &bob, "/api/v1/me/privacy").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["allow_cloud_history"], true);
    assert!(body["history_retention_days"].is_null());
}

#[tokio::test]
async fn endpoints_require_session() {
    let context = test_context().await;
    let router = router_for(&context);

    for (method, uri) in [
        (Method::GET, "/api/v1/me/preferences"),
        (Method::PUT, "/api/v1/me/preferences"),
        (Method::GET, "/api/v1/me/privacy"),
        (Method::PUT, "/api/v1/me/privacy"),
    ] {
        let body = (method == Method::PUT).then(|| json!({ "theme": "dark" }));
        let (status, json) = call(&router, None, method.clone(), uri, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert_eq!(json["code"], "unauthorized", "{method} {uri}");
    }
}

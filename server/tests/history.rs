//! Testes de integração HTTP do histórico de audição (M2e).
//!
//! Tudo roda em `TempDir`: banco SQLite migrado na hora, sem rede.

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tempfile::TempDir;
use tower::ServiceExt;
use zuno_server::auth::Role;
use zuno_server::domain::{AudioFileDraft, TrackDraft, normalize_key};
use zuno_server::repository;
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

fn track_draft(id: &str, title: &str, album_id: &str, artist_id: &str) -> TrackDraft {
    TrackDraft {
        id: id.to_string(),
        album_id: Some(album_id.to_string()),
        title: title.to_string(),
        normalized_title: normalize_key(title),
        disc_no: Some(1),
        track_no: Some(1),
        duration_ms: Some(180_000),
        artists: vec![(artist_id.to_string(), "performer".to_string(), 0)],
    }
}

fn audio_file_draft(id: &str, track_id: &str) -> AudioFileDraft {
    AudioFileDraft {
        id: id.to_string(),
        track_id: track_id.to_string(),
        relative_path: format!("teste/{id}.opus"),
        sha256: format!("sha-{id}"),
        codec: "opus".to_string(),
        container: "ogg".to_string(),
        bitrate: Some(128),
        sample_rate: Some(48_000),
        channels: Some(2),
        size_bytes: 4_096,
        source_mtime: 1_700_000_000,
        available: true,
    }
}

/// Cria artista, álbum e três faixas disponíveis.
async fn seed(pool: &SqlitePool) {
    let artist_id = repository::upsert_artist(pool, "Artista Teste")
        .await
        .expect("artista");
    let album_id =
        repository::upsert_album(pool, "Álbum Teste", Some(&artist_id), Some(2020), None)
            .await
            .expect("álbum");

    for (index, track_id) in ["trk-1", "trk-2", "trk-3"].into_iter().enumerate() {
        let draft = track_draft(track_id, &format!("Faixa {index}"), &album_id, &artist_id);
        let file = audio_file_draft(&format!("file-{index}"), track_id);
        repository::persist_track_with_file(pool, &draft, &file, None)
            .await
            .expect("persistência da faixa");
    }
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

async fn post(router: &Router, cookie: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    call(router, Some(cookie), Method::POST, uri, Some(body)).await
}

async fn delete(router: &Router, cookie: &str, uri: &str) -> (StatusCode, Value) {
    call(router, Some(cookie), Method::DELETE, uri, None).await
}

fn item_ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["id"].as_str().expect("id textual").to_string())
        .collect()
}

async fn record_event(router: &Router, cookie: &str, track_id: &str, kind: &str) -> String {
    let (status, body) = post(
        router,
        cookie,
        "/api/v1/me/history",
        json!({ "track_id": track_id, "event_kind": kind, "seconds": 12.5 }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{track_id}/{kind}: {body}");
    body["id"].as_str().expect("id do evento").to_string()
}

#[tokio::test]
async fn records_play_skip_and_complete() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let router = router_for(&context);
    let cookie = context.cookie.clone();

    // Cada tipo de evento é aceito e devolve o id recém-criado.
    for (track_id, kind) in [("trk-1", "play"), ("trk-2", "skip"), ("trk-3", "complete")] {
        let id = record_event(&router, &cookie, track_id, kind).await;
        assert!(!id.is_empty());
    }

    let (status, page) = get(&router, &cookie, "/api/v1/me/history").await;
    assert_eq!(status, StatusCode::OK);
    let items = page["items"].as_array().expect("items");
    assert_eq!(items.len(), 3);
    assert!(page["next_cursor"].is_null());

    let mut kinds: Vec<&str> = items
        .iter()
        .map(|item| item["event_kind"].as_str().expect("event_kind"))
        .collect();
    kinds.sort_unstable();
    assert_eq!(kinds, vec!["complete", "play", "skip"]);

    for item in items {
        assert!(item["id"].is_string());
        assert_eq!(item["track"]["album"]["title"], "Álbum Teste");
        assert_eq!(item["track"]["artists"][0]["name"], "Artista Teste");
        assert_eq!(item["track"]["available"], true);
        assert!(item["occurred_at"].is_string());
        assert_eq!(item["seconds"], 12.5);
    }

    // `seconds` é opcional.
    let (status, body) = post(
        &router,
        &cookie,
        "/api/v1/me/history",
        json!({ "track_id": "trk-1", "event_kind": "play" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, page) = get(&router, &cookie, "/api/v1/me/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"].as_array().expect("items").len(), 4);
    assert!(
        page["items"]
            .as_array()
            .expect("items")
            .iter()
            .any(|item| item["seconds"].is_null())
    );
}

#[tokio::test]
async fn invalid_events_are_rejected() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let router = router_for(&context);
    let cookie = context.cookie.clone();

    // Faixa inexistente -> 404.
    let (status, body) = post(
        &router,
        &cookie,
        "/api/v1/me/history",
        json!({ "track_id": "nao-existe", "event_kind": "play" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");

    // event_kind fora do enum -> 400.
    let (status, body) = post(
        &router,
        &cookie,
        "/api/v1/me/history",
        json!({ "track_id": "trk-1", "event_kind": "pause" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid_query");

    // seconds fora de 0..=86400 -> 400.
    for seconds in [-0.5, 86_400.5, 1_000_000.0] {
        let (status, body) = post(
            &router,
            &cookie,
            "/api/v1/me/history",
            json!({ "track_id": "trk-1", "event_kind": "play", "seconds": seconds }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "seconds {seconds}");
        assert_eq!(body["code"], "invalid_query", "seconds {seconds}");
    }

    // Nenhum evento inválido foi persistido.
    let (status, page) = get(&router, &cookie, "/api/v1/me/history").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page["items"].as_array().expect("items").is_empty());
}

#[tokio::test]
async fn list_is_paginated_by_cursor_without_repetition() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let router = router_for(&context);
    let cookie = context.cookie.clone();

    let mut created = Vec::new();
    for (track_id, kind) in [("trk-1", "play"), ("trk-2", "skip"), ("trk-3", "complete")] {
        created.push(record_event(&router, &cookie, track_id, kind).await);
    }

    let (status, first) = get(&router, &cookie, "/api/v1/me/history?limit=2").await;
    assert_eq!(status, StatusCode::OK);
    let first_ids = item_ids(&first);
    assert_eq!(first_ids.len(), 2);
    let cursor = first["next_cursor"]
        .as_str()
        .expect("next_cursor na primeira página")
        .to_string();

    let (status, second) = get(
        &router,
        &cookie,
        &format!("/api/v1/me/history?limit=2&cursor={cursor}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let second_ids = item_ids(&second);
    assert_eq!(second_ids.len(), 1);
    assert!(second["next_cursor"].is_null());

    // Sem repetição e sem lacunas: as duas páginas cobrem exatamente os 3 eventos.
    let mut seen = first_ids.clone();
    seen.extend(second_ids);
    assert_eq!(seen.len(), 3, "páginas não devem repetir itens");
    let mut unique = seen.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 3);

    let mut expected = created;
    expected.sort_unstable();
    let mut actual = seen;
    actual.sort_unstable();
    assert_eq!(actual, expected);

    // Cursor malformado -> 400 invalid_cursor.
    let (status, body) = get(&router, &cookie, "/api/v1/me/history?cursor=invalido").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid_cursor");
}

#[tokio::test]
async fn history_is_isolated_and_delete_only_clears_own() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let router = router_for(&context);
    let alice = context.cookie.clone();
    let bob = common::user_cookie(context.db.pool(), "bob", Role::User).await;

    let alice_first = record_event(&router, &alice, "trk-1", "play").await;
    let alice_second = record_event(&router, &alice, "trk-2", "skip").await;
    let bob_event = record_event(&router, &bob, "trk-3", "complete").await;

    let (status, page) = get(&router, &alice, "/api/v1/me/history").await;
    assert_eq!(status, StatusCode::OK);
    let mut alice_ids = item_ids(&page);
    alice_ids.sort_unstable();
    let mut expected = vec![alice_first.clone(), alice_second.clone()];
    expected.sort_unstable();
    assert_eq!(alice_ids, expected);
    assert!(
        !alice_ids.contains(&bob_event),
        "Alice não vê o evento de Bob"
    );

    let (status, page) = get(&router, &bob, "/api/v1/me/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item_ids(&page), vec![bob_event.clone()]);

    // DELETE limpa apenas o histórico de Alice.
    let (status, body) = delete(&router, &alice, "/api/v1/me/history").await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let (status, page) = get(&router, &alice, "/api/v1/me/history").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page["items"].as_array().expect("items").is_empty());

    let (status, page) = get(&router, &bob, "/api/v1/me/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item_ids(&page), vec![bob_event]);

    // DELETE é idempotente.
    let (status, _) = delete(&router, &alice, "/api/v1/me/history").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn endpoints_require_session() {
    let context = test_context().await;
    let router = router_for(&context);

    for (method, uri) in [
        (Method::GET, "/api/v1/me/history"),
        (Method::POST, "/api/v1/me/history"),
        (Method::DELETE, "/api/v1/me/history"),
    ] {
        let body =
            (method == Method::POST).then(|| json!({ "track_id": "trk-1", "event_kind": "play" }));
        let (status, json) = call(&router, None, method.clone(), uri, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert_eq!(json["code"], "unauthorized", "{method} {uri}");
    }
}

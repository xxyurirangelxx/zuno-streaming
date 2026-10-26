//! Testes de integração HTTP de playlists e favoritos (M2c).
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

struct Fixture {
    album_id: String,
    artist_id: String,
}

/// Cria artista, álbum e três faixas disponíveis.
async fn seed(pool: &SqlitePool) -> Fixture {
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

    Fixture {
        album_id,
        artist_id,
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

async fn patch(router: &Router, cookie: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    call(router, Some(cookie), Method::PATCH, uri, Some(body)).await
}

async fn put(router: &Router, cookie: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    call(router, Some(cookie), Method::PUT, uri, body).await
}

async fn delete(router: &Router, cookie: &str, uri: &str) -> (StatusCode, Value) {
    call(router, Some(cookie), Method::DELETE, uri, None).await
}

fn item_ids(detail: &Value) -> Vec<String> {
    detail["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["id"].as_str().expect("id textual").to_string())
        .collect()
}

async fn create_playlist(router: &Router, cookie: &str, name: &str) -> String {
    let (status, body) = post(router, cookie, "/api/v1/playlists", json!({ "name": name })).await;
    assert_eq!(status, StatusCode::CREATED, "criação da playlist: {body}");
    body["id"].as_str().expect("id da playlist").to_string()
}

#[tokio::test]
async fn playlist_crud_and_duplicate_name() {
    let context = test_context().await;
    let router = router_for(&context);
    let cookie = context.cookie.clone();

    // Lista inicial vazia.
    let (status, body) = get(&router, &cookie, "/api/v1/playlists").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().expect("lista").is_empty());

    // Cria.
    let (status, created) = post(
        &router,
        &cookie,
        "/api/v1/playlists",
        json!({ "name": "  Minhas Favoritas  " }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["name"], "Minhas Favoritas");
    assert_eq!(created["visibility"], "private");
    assert_eq!(created["track_count"], 0);
    assert!(created["updated_at"].is_string());
    let playlist_id = created["id"].as_str().expect("id").to_string();

    // Duplicado (nome normalizado igual) -> 409.
    let (status, body) = post(
        &router,
        &cookie,
        "/api/v1/playlists",
        json!({ "name": "minhas   favoritas" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "conflict");

    // Nome vazio -> 400.
    let (status, body) = post(
        &router,
        &cookie,
        "/api/v1/playlists",
        json!({ "name": "   " }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid_query");

    // Lista contém a playlist.
    let (status, body) = get(&router, &cookie, "/api/v1/playlists").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().expect("lista").len(), 1);
    assert_eq!(body[0]["id"], playlist_id.as_str());

    // PATCH nome + visibilidade.
    let (status, updated) = patch(
        &router,
        &cookie,
        &format!("/api/v1/playlists/{playlist_id}"),
        json!({ "name": "Viagem", "visibility": "unlisted" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], "Viagem");
    assert_eq!(updated["visibility"], "unlisted");
    assert_eq!(updated["track_count"], 0);

    // PATCH visibilidade inválida -> 400.
    let (status, body) = patch(
        &router,
        &cookie,
        &format!("/api/v1/playlists/{playlist_id}"),
        json!({ "visibility": "public" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid_query");

    // Detalhe.
    let (status, detail) = get(
        &router,
        &cookie,
        &format!("/api/v1/playlists/{playlist_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["name"], "Viagem");
    assert!(item_ids(&detail).is_empty());

    // DELETE e 404 depois.
    let (status, _) = delete(
        &router,
        &cookie,
        &format!("/api/v1/playlists/{playlist_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = get(
        &router,
        &cookie,
        &format!("/api/v1/playlists/{playlist_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
}

#[tokio::test]
async fn playlist_of_another_user_is_not_found() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let router = router_for(&context);
    let owner = context.cookie.clone();
    let other = common::user_cookie(context.db.pool(), "outro", Role::User).await;

    let playlist_id = create_playlist(&router, &owner, "Privada").await;
    let base = format!("/api/v1/playlists/{playlist_id}");

    let (status, _) = get(&router, &other, &base).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = patch(&router, &other, &base, json!({ "name": "Invasão" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = delete(&router, &other, &base).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = post(
        &router,
        &other,
        &format!("{base}/items"),
        json!({ "track_id": "trk-1" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = delete(&router, &other, &format!("{base}/items/trk-1")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = put(
        &router,
        &other,
        &format!("{base}/items/trk-1/position"),
        Some(json!({ "position": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A playlist continua intacta para o dono.
    let (status, body) = get(&router, &owner, "/api/v1/playlists").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().expect("lista").len(), 1);

    // Outro usuário não vê a playlist na própria lista.
    let (status, body) = get(&router, &other, "/api/v1/playlists").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().expect("lista").is_empty());
}

#[tokio::test]
async fn items_add_duplicate_remove_compacts_and_reorder() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let router = router_for(&context);
    let cookie = context.cookie.clone();

    let playlist_id = create_playlist(&router, &cookie, "Ordem").await;
    let base = format!("/api/v1/playlists/{playlist_id}");

    for (track_id, expected_position) in [("trk-1", 0), ("trk-2", 1), ("trk-3", 2)] {
        let (status, body) = post(
            &router,
            &cookie,
            &format!("{base}/items"),
            json!({ "track_id": track_id }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{track_id}: {body}");
        assert_eq!(body["position"], expected_position);
    }

    // Faixa duplicada -> 409.
    let (status, body) = post(
        &router,
        &cookie,
        &format!("{base}/items"),
        json!({ "track_id": "trk-1" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "conflict");

    // Faixa inexistente -> 404.
    let (status, _) = post(
        &router,
        &cookie,
        &format!("{base}/items"),
        json!({ "track_id": "nao-existe" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, detail) = get(&router, &cookie, &base).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item_ids(&detail), vec!["trk-1", "trk-2", "trk-3"]);
    assert_eq!(detail["track_count"], 3);
    assert_eq!(detail["items"][0]["title"], "Faixa 0");

    // Remover a faixa do meio recompacta 0..n-1.
    let (status, _) = delete(&router, &cookie, &format!("{base}/items/trk-2")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, detail) = get(&router, &cookie, &base).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item_ids(&detail), vec!["trk-1", "trk-3"]);
    assert_eq!(detail["track_count"], 2);

    // Remover faixa que não está na playlist -> 404.
    let (status, _) = delete(&router, &cookie, &format!("{base}/items/trk-2")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Reordena: trk-3 para o início.
    let (status, _) = put(
        &router,
        &cookie,
        &format!("{base}/items/trk-3/position"),
        Some(json!({ "position": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, detail) = get(&router, &cookie, &base).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item_ids(&detail), vec!["trk-3", "trk-1"]);

    // Reordena devolvendo trk-3 para o fim.
    let (status, _) = put(
        &router,
        &cookie,
        &format!("{base}/items/trk-3/position"),
        Some(json!({ "position": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, detail) = get(&router, &cookie, &base).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item_ids(&detail), vec!["trk-1", "trk-3"]);

    // Posições fora de 0..len-1 -> 400 invalid_query.
    for position in [-1, 2, 99] {
        let (status, body) = put(
            &router,
            &cookie,
            &format!("{base}/items/trk-1/position"),
            Some(json!({ "position": position })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "posição {position}");
        assert_eq!(body["code"], "invalid_query");
    }

    // Faixa que não está na playlist -> 404.
    let (status, _) = put(
        &router,
        &cookie,
        &format!("{base}/items/nao-existe/position"),
        Some(json!({ "position": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn favorites_are_idempotent_grouped_and_isolated() {
    let context = test_context().await;
    let fixture = seed(context.db.pool()).await;
    let router = router_for(&context);
    let alice = context.cookie.clone();
    let bob = common::user_cookie(context.db.pool(), "bob", Role::User).await;

    let track_uri = "/api/v1/favorites/track/trk-1";
    let album_uri = format!("/api/v1/favorites/album/{}", fixture.album_id);
    let artist_uri = format!("/api/v1/favorites/artist/{}", fixture.artist_id);

    for uri in [track_uri, album_uri.as_str(), artist_uri.as_str()] {
        let (status, _) = put(&router, &alice, uri, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{uri}");
        // Idempotente.
        let (status, _) = put(&router, &alice, uri, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{uri} (repetido)");
    }

    let (status, body) = get(&router, &alice, "/api/v1/favorites").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tracks"][0]["id"], "trk-1");
    assert_eq!(body["tracks"][0]["title"], "Faixa 0");
    assert_eq!(body["albums"][0]["id"], fixture.album_id.as_str());
    assert_eq!(body["albums"][0]["title"], "Álbum Teste");
    assert_eq!(body["artists"][0]["id"], fixture.artist_id.as_str());
    assert_eq!(body["artists"][0]["name"], "Artista Teste");

    // Isolamento: o favorito de Alice não aparece para Bob.
    let (status, body) = get(&router, &bob, "/api/v1/favorites").await;
    assert_eq!(status, StatusCode::OK);
    for key in ["tracks", "albums", "artists"] {
        assert!(
            body[key].as_array().expect("lista").is_empty(),
            "Bob não deveria ver {key} de Alice"
        );
    }

    // DELETE idempotente.
    for _ in 0..2 {
        let (status, _) = delete(&router, &alice, track_uri).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    let (status, body) = get(&router, &alice, "/api/v1/favorites").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["tracks"].as_array().expect("tracks").is_empty());
    assert_eq!(body["albums"].as_array().expect("albums").len(), 1);

    // Entidade inexistente -> 404; tipo inválido -> 400.
    let (status, _) = put(&router, &alice, "/api/v1/favorites/track/nao-existe", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = put(&router, &alice, "/api/v1/favorites/playlist/trk-1", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid_query");
}

#[tokio::test]
async fn endpoints_require_session() {
    let context = test_context().await;
    let router = router_for(&context);

    for (method, uri) in [
        (Method::GET, "/api/v1/playlists"),
        (Method::POST, "/api/v1/playlists"),
        (Method::GET, "/api/v1/favorites"),
        (Method::PUT, "/api/v1/favorites/track/trk-1"),
        (Method::GET, "/api/v1/playlists/qualquer"),
    ] {
        let body = (method == Method::POST).then(|| json!({ "name": "Sem sessão" }));
        let (status, json) = call(&router, None, method.clone(), uri, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert_eq!(json["code"], "unauthorized", "{method} {uri}");
    }
}

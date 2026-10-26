//! Testes de integração HTTP dos endpoints de listagem/busca (M1b).
//!
//! Tudo roda em `TempDir`: banco SQLite migrado na hora, sem rede.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::SqlitePool;
use tempfile::TempDir;
use tower::ServiceExt;
use zuno_server::domain::{AudioFileDraft, TrackDraft, normalize_key};
use zuno_server::repository::{self, ArtworkDraft};
use zuno_server::{AppState, Config, Database, build_router};

struct TestContext {
    _dir: TempDir,
    db: Database,
    config: Config,
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
        db_max_connections: 2,
    };

    let db = Database::connect(&config)
        .await
        .expect("banco de teste deve conectar");

    TestContext {
        _dir: dir,
        db,
        config,
    }
}

fn app_for(context: &TestContext) -> axum::Router {
    build_router(AppState::new(
        context.db.pool().clone(),
        context.config.clone(),
        "0.1.0",
    ))
}

fn track_draft(
    id: &str,
    title: &str,
    album_id: Option<String>,
    artists: Vec<(String, u32)>,
) -> TrackDraft {
    TrackDraft {
        id: id.to_string(),
        album_id,
        title: title.to_string(),
        normalized_title: normalize_key(title),
        disc_no: Some(1),
        track_no: Some(1),
        duration_ms: Some(180_000),
        artists: artists
            .into_iter()
            .map(|(artist_id, sort_order)| (artist_id, "performer".to_string(), sort_order))
            .collect(),
    }
}

fn audio_file_draft(
    id: &str,
    track_id: &str,
    relative_path: &str,
    available: bool,
) -> AudioFileDraft {
    AudioFileDraft {
        id: id.to_string(),
        track_id: track_id.to_string(),
        relative_path: relative_path.to_string(),
        sha256: format!("sha-{id}"),
        codec: "opus".to_string(),
        container: "ogg".to_string(),
        bitrate: Some(128),
        sample_rate: Some(48_000),
        channels: Some(2),
        size_bytes: 4_096,
        source_mtime: 1_700_000_000,
        available,
    }
}

fn artwork_draft() -> ArtworkDraft {
    ArtworkDraft {
        relative_path: "capa-chega.jpg".to_string(),
        source: "embedded".to_string(),
        mime_type: Some("image/jpeg".to_string()),
        checksum: "capa-chega".to_string(),
    }
}

/// Biblioteca de teste usada pela maioria dos casos.
///
/// Faixas (ordem keyset = `normalized_title, id`, colação BINARY do SQLite):
/// 1. `trk-a` "Canção de Amor" — álbum "Chega de Saudade", João Gilberto, com arquivo disponível
/// 2. `trk-b` "Chega de Saudade" — mesmo álbum, arquivo disponível
/// 3. `trk-c` "Águas de Março" — álbum "Elis & Tom", Elis (0) + João (1)
/// 4. `trk-d` "Sem Album" — sem álbum, arquivo indexado mas indisponível
/// 5. `trk-e` "Zulu" — sem álbum e sem nenhum arquivo
///
/// Como "águas" começa com byte UTF-8 >= 0xC3, ela fica por último na colação.
struct Fixture {
    joao: String,
    elis: String,
    chega: String,
    elis_tom: String,
}

async fn seed(pool: &SqlitePool) -> Fixture {
    let joao = repository::upsert_artist(pool, "João Gilberto")
        .await
        .expect("artista João");
    let elis = repository::upsert_artist(pool, "Elis Regina")
        .await
        .expect("artista Elis");

    let chega = repository::upsert_album(pool, "Chega de Saudade", Some(&joao), Some(1959), None)
        .await
        .expect("álbum Chega de Saudade");
    let elis_tom = repository::upsert_album(pool, "Elis & Tom", Some(&elis), Some(1974), None)
        .await
        .expect("álbum Elis & Tom");

    let tracks = [
        (
            track_draft(
                "trk-a",
                "Canção de Amor",
                Some(chega.clone()),
                vec![(joao.clone(), 0)],
            ),
            audio_file_draft("file-a", "trk-a", "joao/01.opus", true),
            Some(artwork_draft()),
        ),
        (
            track_draft(
                "trk-b",
                "Chega de Saudade",
                Some(chega.clone()),
                vec![(joao.clone(), 0)],
            ),
            audio_file_draft("file-b", "trk-b", "joao/02.opus", true),
            None,
        ),
        (
            track_draft(
                "trk-c",
                "Águas de Março",
                Some(elis_tom.clone()),
                vec![(elis.clone(), 0), (joao.clone(), 1)],
            ),
            audio_file_draft("file-c", "trk-c", "elis/01.opus", true),
            None,
        ),
        (
            track_draft("trk-d", "Sem Album", None, vec![(elis.clone(), 0)]),
            audio_file_draft("file-d", "trk-d", "elis/02.opus", false),
            None,
        ),
        (
            track_draft("trk-e", "Zulu", None, vec![(elis.clone(), 0)]),
            audio_file_draft("file-e", "trk-e", "elis/03.opus", false),
            None,
        ),
    ];

    for (track, file, artwork) in &tracks {
        repository::persist_track_with_file(pool, track, file, artwork.as_ref())
            .await
            .expect("persistência da faixa");
    }

    // `trk-e` existe na biblioteca sem nenhum arquivo indexado.
    sqlx::query("DELETE FROM audio_files WHERE track_id = ?1")
        .bind("trk-e")
        .execute(pool)
        .await
        .expect("remoção do arquivo de teste");

    Fixture {
        joao,
        elis,
        chega,
        elis_tom,
    }
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("resposta");

    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("corpo da resposta")
        .to_bytes();
    let json = serde_json::from_slice(&bytes).expect("JSON válido");

    (status, json)
}

/// Ids de uma lista JSON qualquer (página ou categoria da busca).
fn list_ids(list: &Value) -> Vec<String> {
    list.as_array()
        .expect("lista")
        .iter()
        .map(|item| item["id"].as_str().expect("id textual").to_string())
        .collect()
}

/// Ids de uma página paginada (`items`).
fn ids(json: &Value) -> Vec<String> {
    list_ids(&json["items"])
}

#[tokio::test]
async fn list_tracks_returns_explicit_summaries_in_keyset_order() {
    let context = test_context().await;
    let fixture = seed(context.db.pool()).await;
    let app = app_for(&context);

    let (status, json) = get_json(&app, "/api/v1/tracks").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        ids(&json),
        vec!["trk-a", "trk-b", "trk-d", "trk-e", "trk-c"],
        "ordem por normalized_title, id"
    );
    assert_eq!(json["next_cursor"], Value::Null);

    let items = json["items"].as_array().expect("items");

    // Faixa completa: álbum, artista performer, capa do álbum e disponibilidade.
    let first = &items[0];
    assert_eq!(first["title"], "Canção de Amor");
    assert_eq!(first["album"]["id"], fixture.chega.as_str());
    assert_eq!(first["album"]["title"], "Chega de Saudade");
    assert_eq!(first["artists"][0]["id"], fixture.joao.as_str());
    assert_eq!(first["artists"][0]["name"], "João Gilberto");
    assert!(first["artwork_id"].is_string(), "capa do álbum propagada");
    assert_eq!(first["available"], Value::Bool(true));
    assert_eq!(first["duration_ms"], 180_000);

    // Artistas em ordem de `sort_order`.
    let duet = &items[4];
    assert_eq!(duet["id"], "trk-c");
    assert_eq!(duet["artists"][0]["name"], "Elis Regina");
    assert_eq!(duet["artists"][1]["name"], "João Gilberto");
    assert_eq!(duet["artwork_id"], Value::Null);

    // Faixa sem álbum e sem arquivo disponível.
    let homeless = &items[2];
    assert_eq!(homeless["album"], Value::Null);
    assert_eq!(homeless["available"], Value::Bool(false));

    let without_file = &items[3];
    assert_eq!(without_file["available"], Value::Bool(false));
    assert!(without_file["artists"][0]["name"].is_string());
}

#[tokio::test]
async fn pagination_walks_pages_without_repeating_items() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let app = app_for(&context);

    let (status, page1) = get_json(&app, "/api/v1/tracks?limit=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&page1), vec!["trk-a", "trk-b"]);

    let cursor1 = page1["next_cursor"]
        .as_str()
        .expect("primeira página tem cursor")
        .to_string();

    let (status, page2) = get_json(&app, &format!("/api/v1/tracks?limit=2&cursor={cursor1}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&page2), vec!["trk-d", "trk-e"]);

    let cursor2 = page2["next_cursor"]
        .as_str()
        .expect("segunda página tem cursor")
        .to_string();

    let (status, page3) = get_json(&app, &format!("/api/v1/tracks?limit=2&cursor={cursor2}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&page3), vec!["trk-c"]);
    assert_eq!(
        page3["next_cursor"],
        Value::Null,
        "última página sem cursor"
    );

    let mut all = ids(&page1);
    all.extend(ids(&page2));
    all.extend(ids(&page3));
    let mut unique = all.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(all.len(), unique.len(), "nenhuma faixa repetida");
    assert_eq!(all.len(), 5);
}

#[tokio::test]
async fn list_tracks_filters_by_album_and_artist() {
    let context = test_context().await;
    let fixture = seed(context.db.pool()).await;
    let app = app_for(&context);

    let (status, by_album) =
        get_json(&app, &format!("/api/v1/tracks?album_id={}", fixture.chega)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&by_album), vec!["trk-a", "trk-b"]);

    let (status, by_joao) =
        get_json(&app, &format!("/api/v1/tracks?artist_id={}", fixture.joao)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&by_joao), vec!["trk-a", "trk-b", "trk-c"]);

    let (status, by_elis) =
        get_json(&app, &format!("/api/v1/tracks?artist_id={}", fixture.elis)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&by_elis), vec!["trk-d", "trk-e", "trk-c"]);

    // Filtros combinados com `q` continuam coerentes com o keyset.
    let (status, filtered) = get_json(
        &app,
        &format!("/api/v1/tracks?artist_id={}&q=cancao", fixture.joao),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&filtered), vec!["trk-a"]);

    let (status, empty) = get_json(
        &app,
        &format!(
            "/api/v1/tracks?album_id={}&artist_id={}",
            fixture.elis_tom, fixture.joao
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&empty), vec!["trk-c"]);
}

#[tokio::test]
async fn track_detail_returns_preferred_audio_and_404_for_missing_track() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let app = app_for(&context);

    let (status, json) = get_json(&app, "/api/v1/tracks/trk-a").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["id"], "trk-a");
    assert_eq!(json["title"], "Canção de Amor");
    assert_eq!(json["available"], Value::Bool(true));
    assert_eq!(json["audio"]["codec"], "opus");
    assert_eq!(json["audio"]["container"], "ogg");
    assert_eq!(json["audio"]["bitrate"], 128);
    assert_eq!(json["audio"]["sample_rate"], 48_000);
    assert_eq!(json["audio"]["channels"], 2);
    assert_eq!(json["audio"]["size_bytes"], 4_096);
    assert_eq!(json["audio"]["available"], Value::Bool(true));

    // Faixa sem nenhum arquivo indexado: `audio` nulo, detalhe ainda existe.
    let (status, json) = get_json(&app, "/api/v1/tracks/trk-e").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["audio"], Value::Null);

    let (status, json) = get_json(&app, "/api/v1/tracks/nao-existe").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["code"], "not_found");
}

#[tokio::test]
async fn albums_report_artist_year_and_track_count() {
    let context = test_context().await;
    let fixture = seed(context.db.pool()).await;
    let app = app_for(&context);

    let (status, json) = get_json(&app, "/api/v1/albums").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        ids(&json),
        vec![fixture.chega.clone(), fixture.elis_tom.clone()],
        "ordem por normalized_title"
    );
    assert_eq!(json["next_cursor"], Value::Null);

    let albums = json["items"].as_array().expect("items");
    assert_eq!(albums[0]["title"], "Chega de Saudade");
    assert_eq!(albums[0]["artist"]["name"], "João Gilberto");
    assert_eq!(albums[0]["release_year"], 1959);
    assert_eq!(albums[0]["track_count"], 2);
    assert!(albums[0]["artwork_id"].is_string());

    assert_eq!(albums[1]["title"], "Elis & Tom");
    assert_eq!(albums[1]["artist"]["name"], "Elis Regina");
    assert_eq!(albums[1]["release_year"], 1974);
    assert_eq!(albums[1]["track_count"], 1);
    assert_eq!(albums[1]["artwork_id"], Value::Null);
}

#[tokio::test]
async fn artists_report_track_and_album_counts() {
    let context = test_context().await;
    let fixture = seed(context.db.pool()).await;
    let app = app_for(&context);

    let (status, json) = get_json(&app, "/api/v1/artists").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        ids(&json),
        vec![fixture.elis.clone(), fixture.joao.clone()],
        "ordem por normalized_name"
    );

    let artists = json["items"].as_array().expect("items");
    assert_eq!(artists[0]["name"], "Elis Regina");
    assert_eq!(artists[0]["track_count"], 3);
    assert_eq!(
        artists[0]["album_count"], 1,
        "album_artist de 'Elis & Tom' e participa de faixas do mesmo álbum"
    );

    assert_eq!(artists[1]["name"], "João Gilberto");
    assert_eq!(artists[1]["track_count"], 3);
    assert_eq!(
        artists[1]["album_count"], 2,
        "album_artist de 'Chega de Saudade' e performer em 'Elis & Tom'"
    );
}

#[tokio::test]
async fn search_finds_tracks_without_accents_and_returns_albums_and_artists() {
    let context = test_context().await;
    let fixture = seed(context.db.pool()).await;
    let app = app_for(&context);

    // Título com acento encontrado sem acento (FTS com remove_diacritics).
    let (status, json) = get_json(&app, "/api/v1/search?q=cancao").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["query"], "cancao");
    assert_eq!(
        list_ids(&json["tracks"]).as_slice(),
        &["trk-a".to_string()][..]
    );

    let (status, json) = get_json(&app, "/api/v1/search?q=aguas").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        list_ids(&json["tracks"]).as_slice(),
        &["trk-c".to_string()][..]
    );

    // Artista sem acento encontra as faixas e o próprio artista.
    let (status, json) = get_json(&app, "/api/v1/search?q=joao").await;
    assert_eq!(status, StatusCode::OK);
    let mut tracks = list_ids(&json["tracks"]);
    tracks.sort();
    assert_eq!(tracks, vec!["trk-a", "trk-b", "trk-c"]);

    // Busca pelo alfabeto do álbum retorna álbum + faixa.
    let (status, json) = get_json(&app, "/api/v1/search?q=chega").await;
    assert_eq!(status, StatusCode::OK);
    let mut by_album = list_ids(&json["tracks"]);
    by_album.sort();
    assert_eq!(
        by_album,
        vec!["trk-a", "trk-b"],
        "o FTS indexa o título do álbum junto com a faixa"
    );
    assert_eq!(
        list_ids(&json["albums"]).as_slice(),
        &[fixture.chega.clone()][..]
    );
    assert_eq!(json["albums"][0]["track_count"], 2);
    assert!(json["artists"].as_array().expect("artists").is_empty());

    // Busca por artista: prefixo case-insensitive no nome normalizado.
    let (status, json) = get_json(&app, "/api/v1/search?q=Elis").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        list_ids(&json["artists"]).as_slice(),
        &[fixture.elis.clone()][..]
    );
    assert_eq!(json["artists"][0]["track_count"], 3);

    // Acento no nome do artista também casa (normalização em minúsculas/UTF-8).
    let (status, json) = get_json(&app, "/api/v1/search?q=jo%C3%A3o").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        list_ids(&json["artists"]).as_slice(),
        &[fixture.joao.clone()][..]
    );

    // Nada encontrado: listas vazias, sem erro.
    let (status, json) = get_json(&app, "/api/v1/search?q=beatles").await;
    assert_eq!(status, StatusCode::OK);
    for key in ["tracks", "albums", "artists"] {
        assert!(
            json[key].as_array().expect("lista").is_empty(),
            "{key} deveria estar vazio"
        );
    }
}

#[tokio::test]
async fn search_with_empty_query_returns_empty_lists() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let app = app_for(&context);

    for uri in [
        "/api/v1/search",
        "/api/v1/search?q=",
        "/api/v1/search?q=%20%20",
    ] {
        let (status, json) = get_json(&app, uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        for key in ["tracks", "albums", "artists"] {
            assert!(
                json[key].as_array().expect("lista").is_empty(),
                "{uri}: {key} deveria estar vazio"
            );
        }
    }
}

#[tokio::test]
async fn invalid_cursor_is_rejected_with_400() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let app = app_for(&context);

    for uri in [
        "/api/v1/tracks?cursor=lixo",
        "/api/v1/albums?cursor=lixo",
        "/api/v1/artists?cursor=zz%3Aabc",
    ] {
        let (status, json) = get_json(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(json["code"], "invalid_cursor", "{uri}");
    }
}

#[tokio::test]
async fn limit_above_maximum_is_clamped() {
    let context = test_context().await;
    seed(context.db.pool()).await;
    let app = app_for(&context);

    let (status, json) = get_json(&app, "/api/v1/tracks?limit=1000").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        ids(&json).len(),
        5,
        "todas as faixas cabem no limite máximo"
    );
    assert_eq!(json["next_cursor"], Value::Null);

    let (status, json) = get_json(&app, "/api/v1/tracks?limit=0").await;
    assert_eq!(status, StatusCode::OK, "limite zero é clampado para 1");
    assert_eq!(ids(&json).len(), 1);
    assert!(
        json["next_cursor"].is_string(),
        "ainda há páginas depois do mínimo"
    );
}

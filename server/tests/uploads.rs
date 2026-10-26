//! Testes de integração HTTP do fluxo de upload com staging (M1c).
//!
//! Tudo roda em `TempDir`: banco migrado na hora, staging e biblioteca em
//! diretórios próprios, sem rede.

use std::fs;
use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tower::ServiceExt;
use zuno_server::{AppState, Config, Database, build_router};

mod common;

struct TestEnv {
    dir: TempDir,
    database: Database,
    config: Config,
    cookie: String,
}

struct AuthedApp {
    router: Router,
    cookie: String,
}

impl TestEnv {
    async fn new(max_audio_file_bytes: u64) -> Self {
        let dir = tempfile::tempdir().expect("diretório temporário");
        let music_dir = dir.path().join("music");
        fs::create_dir_all(&music_dir).expect("criar diretório de música");

        let database_url = format!(
            "sqlite://{}",
            dir.path()
                .join("zuno.db")
                .to_string_lossy()
                .replace('\\', "/")
        );

        let config = Config {
            bind_addr: "127.0.0.1:0".parse().expect("bind de teste"),
            database_url,
            music_dir,
            artwork_dir: dir.path().join("artwork"),
            staging_dir: dir.path().join("staging"),
            max_audio_file_bytes,
            scan_on_startup: false,
            cookie_secure: false,
            db_max_connections: 2,
        };

        let database = Database::connect(&config)
            .await
            .expect("banco de teste deve conectar");
        let cookie = common::admin_cookie(database.pool()).await;

        Self {
            dir,
            database,
            config,
            cookie,
        }
    }

    fn app(&self) -> AuthedApp {
        AuthedApp {
            router: build_router(AppState::new(
                self.database.pool().clone(),
                self.config.clone(),
                "0.1.0",
            )),
            cookie: self.cookie.clone(),
        }
    }

    fn staging_files(&self) -> Vec<PathBuf> {
        files_with_extension(&self.config.staging_dir, None)
    }

    fn library_audio(&self) -> Vec<PathBuf> {
        files_with_extension(&self.config.music_dir, Some("opus"))
    }
}

fn files_with_extension(dir: &Path, extension: Option<&str>) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(dir, extension, &mut found);
    found
}

fn collect(dir: &Path, extension: Option<&str>, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, extension, found);
            continue;
        }

        let matches = match extension {
            Some(extension) => path
                .extension()
                .is_some_and(|value| value.eq_ignore_ascii_case(extension)),
            None => true,
        };

        if matches {
            found.push(path);
        }
    }
}

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/audio")
        .join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("fixture {}: {error}", path.display()))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

async fn send(app: &AuthedApp, mut request: Request<Body>) -> (StatusCode, Value) {
    request.headers_mut().insert(
        header::COOKIE,
        app.cookie.parse().expect("cookie de teste válido"),
    );

    let response = app
        .router
        .clone()
        .oneshot(request)
        .await
        .expect("requisição");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("corpo da resposta")
        .to_bytes();

    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("resposta JSON")
    };

    (status, body)
}

async fn upload(app: &AuthedApp, filename: Option<&str>, bytes: Vec<u8>) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/v1/uploads")
        .header("content-type", "application/octet-stream");

    if let Some(filename) = filename {
        builder = builder.header("x-zuno-filename", filename);
    }

    send(app, builder.body(Body::from(bytes)).expect("requisição")).await
}

async fn get_upload(app: &AuthedApp, upload_id: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/uploads/{upload_id}"))
        .body(Body::empty())
        .expect("requisição");
    send(app, request).await
}

async fn complete_upload(app: &AuthedApp, upload_id: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/uploads/{upload_id}/complete"))
        .body(Body::empty())
        .expect("requisição");
    send(app, request).await
}

async fn delete_upload(app: &AuthedApp, upload_id: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("DELETE")
        .uri(format!("/api/v1/uploads/{upload_id}"))
        .body(Body::empty())
        .expect("requisição");
    send(app, request).await
}

async fn get_track(app: &AuthedApp, track_id: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tracks/{track_id}"))
        .body(Body::empty())
        .expect("requisição");
    send(app, request).await
}

#[tokio::test]
async fn upload_and_complete_indexes_track_and_rejects_duplicate() {
    let env = TestEnv::new(10 * 1024 * 1024).await;
    let app = env.app();
    let bytes = fixture("tone-a.opus");
    let expected_sha = sha256(&bytes);

    let (status, body) = upload(&app, Some("tone-a.opus"), bytes.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "corpo: {body}");
    assert_eq!(body["status"], json!("staged"));
    assert_eq!(body["original_filename"], json!("tone-a.opus"));
    assert_eq!(body["sha256"], json!(expected_sha));
    assert_eq!(body["size_bytes"], json!(bytes.len()));

    let upload_id = body["upload_id"].as_str().expect("upload_id").to_string();

    let (status, body) = get_upload(&app, &upload_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], json!("staged"));
    assert_eq!(body["sha256"], json!(expected_sha));

    let (status, body) = complete_upload(&app, &upload_id).await;
    assert_eq!(status, StatusCode::OK, "corpo: {body}");
    assert_eq!(body["status"], json!("indexed"));
    let track_id = body["track_id"].as_str().expect("track_id").to_string();

    assert!(
        env.staging_files().is_empty(),
        "staging deve ficar vazio: {:?}",
        env.staging_files()
    );

    let library = env.library_audio();
    assert_eq!(library.len(), 1, "um arquivo na biblioteca: {library:?}");
    assert!(
        library[0].starts_with(&env.config.music_dir),
        "arquivo deve ficar dentro da biblioteca"
    );

    let (status, body) = get_track(&app, &track_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], json!(track_id));
    assert_eq!(body["available"], json!(true));

    // Mesmo conteúdo novamente: 409 duplicate, sem tocar na biblioteca.
    let (status, body) = upload(&app, Some("tone-a.opus"), bytes).await;
    assert_eq!(status, StatusCode::CREATED);
    let second_id = body["upload_id"].as_str().expect("upload_id").to_string();

    let (status, body) = complete_upload(&app, &second_id).await;
    assert_eq!(status, StatusCode::CONFLICT, "corpo: {body}");
    assert_eq!(body["upload_id"], json!(second_id));
    assert_eq!(body["status"], json!("duplicate"));
    assert_eq!(body["track_id"], json!(track_id));

    assert!(
        env.staging_files().is_empty(),
        "staging do duplicado deve ser descartado"
    );
    assert_eq!(
        env.library_audio().len(),
        1,
        "duplicata não pode criar nem apagar arquivo da biblioteca"
    );
}

#[tokio::test]
async fn rejects_unsupported_invalid_and_unnamed_uploads() {
    let env = TestEnv::new(10 * 1024 * 1024).await;
    let app = env.app();

    let (status, body) = upload(&app, Some("vorbis.ogg"), fixture("vorbis.ogg")).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "corpo: {body}");
    assert_eq!(body["code"], json!("unsupported_format"));

    let garbage: Vec<u8> = (0..512_u32).map(|value| (value * 37 % 251) as u8).collect();
    let (status, body) = upload(&app, Some("garbage.opus"), garbage).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "corpo: {body}");
    assert_eq!(body["code"], json!("invalid_audio"));

    let (status, body) = upload(&app, None, fixture("tone-a.opus")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "corpo: {body}");
    assert_eq!(body["code"], json!("invalid_filename"));

    assert!(
        env.staging_files().is_empty(),
        "rejeições não podem deixar arquivos no staging: {:?}",
        env.staging_files()
    );
}

#[tokio::test]
async fn oversized_upload_is_rejected_and_staging_is_cleaned() {
    let env = TestEnv::new(1024).await;
    let app = env.app();

    let (status, body) = upload(&app, Some("tone-a.opus"), fixture("tone-a.opus")).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "corpo: {body}");
    assert_eq!(body["code"], json!("too_large"));

    assert!(
        env.staging_files().is_empty(),
        "o `.part` deve ser removido: {:?}",
        env.staging_files()
    );
    assert!(env.library_audio().is_empty());
}

#[tokio::test]
async fn delete_removes_only_the_staged_file() {
    let env = TestEnv::new(10 * 1024 * 1024).await;
    let app = env.app();

    let (status, body) = upload(&app, Some("tone-a.opus"), fixture("tone-a.opus")).await;
    assert_eq!(status, StatusCode::CREATED);
    let upload_id = body["upload_id"].as_str().expect("upload_id").to_string();

    let (status, body) = delete_upload(&app, &upload_id).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "corpo: {body}");
    assert_eq!(env.staging_files().len(), 0);
    assert!(env.library_audio().is_empty());

    let (status, _) = get_upload(&app, &upload_id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A linha foi removida, então completar depois do DELETE responde 404
    // (e não 409 `not_pending`).
    let (status, _) = complete_upload(&app, &upload_id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn malicious_filename_never_escapes_the_library() {
    let env = TestEnv::new(10 * 1024 * 1024).await;
    let app = env.app();

    let (status, body) = upload(&app, Some("../../evil.opus"), fixture("tone-a.opus")).await;
    assert_eq!(status, StatusCode::CREATED, "corpo: {body}");
    let upload_id = body["upload_id"].as_str().expect("upload_id").to_string();

    let (status, body) = complete_upload(&app, &upload_id).await;
    assert_eq!(status, StatusCode::OK, "corpo: {body}");

    let library = env.library_audio();
    assert_eq!(library.len(), 1, "arquivo deve existir: {library:?}");
    assert!(
        library[0].starts_with(&env.config.music_dir),
        "todo arquivo precisa ficar dentro da biblioteca"
    );
    assert!(
        !env.dir.path().join("evil.opus").exists(),
        "nada pode ser criado fora da biblioteca"
    );
}

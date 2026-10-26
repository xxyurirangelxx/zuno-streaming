//! Testes de integração HTTP do M2a: setup, sessão, RBAC, convites e CSRF.

mod common;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;
use zuno_server::auth::{Role, generate_setup_code, hash_token};
use zuno_server::repository::auth as auth_repo;
use zuno_server::{AppState, Config, Database, build_router};

struct TestEnv {
    _dir: TempDir,
    database: Database,
    config: Config,
}

async fn setup() -> TestEnv {
    let dir = tempfile::tempdir().expect("diretório temporário");
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
        music_dir: dir.path().join("music"),
        artwork_dir: dir.path().join("artwork"),
        staging_dir: dir.path().join("staging"),
        max_audio_file_bytes: 10 * 1024 * 1024,
        scan_on_startup: false,
        cookie_secure: false,
        db_max_connections: 2,
    };

    let database = Database::connect(&config)
        .await
        .expect("banco de teste deve conectar");

    TestEnv {
        _dir: dir,
        database,
        config,
    }
}

fn app(env: &TestEnv) -> axum::Router {
    build_router(AppState::new(
        env.database.pool().clone(),
        env.config.clone(),
        "0.1.0",
    ))
}

async fn send(
    env: &TestEnv,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
    payload: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    send_to(app(env), method, uri, headers, payload).await
}

/// Envia por um roteador já construído (preserva o `AppState`, inclusive o
/// limitador de tentativas de login em memória).
async fn send_to(
    router: axum::Router,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
    payload: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }

    let body = match payload {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(serde_json::to_vec(&value).expect("JSON serializável"))
        }
        None => Body::empty(),
    };

    let response = router
        .oneshot(builder.body(body).expect("request"))
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

    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("JSON válido")
    };

    (status, headers, body)
}

/// Extrai apenas `zuno_session=...` do `Set-Cookie` para reutilizar na requisição.
fn session_cookie(headers: &HeaderMap) -> String {
    headers
        .get(header::SET_COOKIE)
        .expect("resposta deve definir cookie")
        .to_str()
        .expect("cookie ASCII")
        .split(';')
        .next()
        .expect("par nome=valor")
        .to_string()
}

#[tokio::test]
async fn setup_flow_creates_admin_and_accepts_a_single_claim() {
    let env = setup().await;

    let (status, _, body) = send(&env, Method::GET, "/api/v1/setup/status", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["required"], json!(true));

    // Código inválido: mesma resposta de credenciais inválidas.
    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/setup/claim",
        &[],
        Some(json!({
            "code": "CODIGO-INVALIDO",
            "username": "admin",
            "password": "senha-forte-1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], json!("invalid_credentials"));

    let code = generate_setup_code();
    auth_repo::insert_setup_token(
        env.database.pool(),
        &Uuid::now_v7().to_string(),
        &hash_token(&code),
        "+1 day",
    )
    .await
    .expect("inserir código de setup");

    let (status, headers, body) = send(
        &env,
        Method::POST,
        "/api/v1/setup/claim",
        &[],
        Some(json!({
            "code": code,
            "username": "admin",
            "password": "senha-forte-1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "corpo: {body}");
    assert_eq!(body["user"]["role"], json!("admin"));
    assert!(
        headers
            .get(header::SET_COOKIE)
            .expect("Set-Cookie no claim")
            .to_str()
            .expect("cookie ASCII")
            .contains("zuno_session"),
        "claim deve autenticar o administrador"
    );

    // Segundo claim: setup já concluído (código consumido e usuário existente).
    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/setup/claim",
        &[],
        Some(json!({
            "code": generate_setup_code(),
            "username": "outro-admin",
            "password": "senha-forte-2"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        body["message"]
            .as_str()
            .expect("mensagem")
            .contains("setup_completed"),
        "corpo: {body}"
    );

    let (status, _, body) = send(&env, Method::GET, "/api/v1/setup/status", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["required"], json!(false));
}

#[tokio::test]
async fn protected_routes_require_a_valid_session() {
    let env = setup().await;

    let (status, _, body) = send(&env, Method::GET, "/api/v1/tracks", &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], json!("unauthorized"));

    let cookie = common::admin_cookie(env.database.pool()).await;
    let (status, _, _) = send(
        &env,
        Method::GET,
        "/api/v1/tracks",
        &[("cookie", cookie.as_str())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn login_rejects_bad_passwords_and_limits_attempts() {
    let env = setup().await;

    // Senha errada para um usuário existente.
    common::user_cookie(env.database.pool(), "login-ok", Role::User).await;
    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/auth/login",
        &[],
        Some(json!({
            "username": "login-ok",
            "password": "senha-que-nao-confere"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], json!("invalid_credentials"));

    // Login correto: cookie de sessão no Set-Cookie.
    let (status, headers, body) = send(
        &env,
        Method::POST,
        "/api/v1/auth/login",
        &[],
        Some(json!({
            "username": "login-ok",
            "password": common::TEST_PASSWORD
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "corpo: {body}");
    assert_eq!(body["user"]["username"], json!("login-ok"));
    assert!(
        headers
            .get(header::SET_COOKIE)
            .expect("Set-Cookie no login")
            .to_str()
            .expect("cookie ASCII")
            .contains("zuno_session")
    );

    // Usuário separado para não contaminar o limitador acima. O mesmo roteador
    // é reutilizado para que o limitador em memória sobreviva entre requisições.
    common::user_cookie(env.database.pool(), "login-limite", Role::User).await;
    let login_router = app(&env);
    for attempt in 1..=zuno_server::auth::LOGIN_ATTEMPT_LIMIT {
        let (status, _, body) = send_to(
            login_router.clone(),
            Method::POST,
            "/api/v1/auth/login",
            &[],
            Some(json!({
                "username": "login-limite",
                "password": "senha-errada"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "tentativa {attempt}");
        assert_eq!(body["code"], json!("invalid_credentials"));
    }

    let (status, _, body) = send_to(
        login_router,
        Method::POST,
        "/api/v1/auth/login",
        &[],
        Some(json!({
            "username": "login-limite",
            "password": "senha-errada"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], json!("too_many_attempts"));
}

#[tokio::test]
async fn refresh_rotates_the_cookie_and_logout_revokes_it() {
    let env = setup().await;
    let old_cookie = common::user_cookie(env.database.pool(), "refresh-user", Role::User).await;

    let (status, headers, body) = send(
        &env,
        Method::POST,
        "/api/v1/auth/refresh",
        &[("cookie", old_cookie.as_str())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "corpo: {body}");
    let new_cookie = session_cookie(&headers);
    assert!(new_cookie.contains("zuno_session"));
    assert_ne!(new_cookie, old_cookie, "refresh deve rotacionar o token");

    // O token rotacionado deixa de valer.
    let (status, _, body) = send(
        &env,
        Method::GET,
        "/api/v1/me",
        &[("cookie", old_cookie.as_str())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], json!("unauthorized"));

    let (status, _, body) = send(
        &env,
        Method::GET,
        "/api/v1/me",
        &[("cookie", new_cookie.as_str())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], json!("refresh-user"));

    let (status, _, _) = send(
        &env,
        Method::POST,
        "/api/v1/auth/logout",
        &[("cookie", new_cookie.as_str())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _, body) = send(
        &env,
        Method::GET,
        "/api/v1/me",
        &[("cookie", new_cookie.as_str())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], json!("unauthorized"));
}

#[tokio::test]
async fn invitations_enforce_admin_role_and_are_single_use() {
    let env = setup().await;
    let admin = common::admin_cookie(env.database.pool()).await;

    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/invitations",
        &[("cookie", admin.as_str())],
        Some(json!({ "role": "user" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "corpo: {body}");
    assert_eq!(body["role"], json!("user"));
    let token = body["token"]
        .as_str()
        .expect("token do convite")
        .to_string();

    // Usuário comum não administra convites.
    let member = common::user_cookie(env.database.pool(), "member", Role::User).await;
    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/invitations",
        &[("cookie", member.as_str())],
        Some(json!({ "role": "user" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], json!("forbidden"));

    // Convite não pode criar administrador.
    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/invitations",
        &[("cookie", admin.as_str())],
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], json!("invalid_query"));

    // Redenção cria usuário comum e autentica.
    let (status, headers, body) = send(
        &env,
        Method::POST,
        "/api/v1/invitations/redeem",
        &[],
        Some(json!({
            "token": token,
            "username": "convidado",
            "password": "senha-convidado"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "corpo: {body}");
    assert_eq!(body["user"]["role"], json!("user"));
    let invited_cookie = session_cookie(&headers);

    let (status, _, body) = send(
        &env,
        Method::GET,
        "/api/v1/me",
        &[("cookie", invited_cookie.as_str())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["role"], json!("user"));

    // Uso único: o mesmo token não pode ser resgatado de novo.
    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/invitations/redeem",
        &[],
        Some(json!({
            "token": token,
            "username": "convidado-2",
            "password": "senha-convidado-2"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], json!("invalid_credentials"));

    // Convite revogado não pode ser resgatado.
    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/invitations",
        &[("cookie", admin.as_str())],
        Some(json!({ "role": "user" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let revoked_id = body["id"].as_str().expect("id do convite").to_string();
    let revoked_token = body["token"]
        .as_str()
        .expect("token do convite revogado")
        .to_string();

    let (status, _, _) = send(
        &env,
        Method::DELETE,
        &format!("/api/v1/invitations/{revoked_id}"),
        &[("cookie", admin.as_str())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/invitations/redeem",
        &[],
        Some(json!({
            "token": revoked_token,
            "username": "convidado-3",
            "password": "senha-convidado-3"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], json!("invalid_credentials"));
}

#[tokio::test]
async fn cross_origin_mutations_are_rejected_even_with_a_cookie() {
    let env = setup().await;
    let admin = common::admin_cookie(env.database.pool()).await;

    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/uploads",
        &[
            ("cookie", admin.as_str()),
            ("host", "localhost"),
            ("origin", "https://evil.example"),
        ],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], json!("forbidden"));

    // Mesma origem: qualquer falha posterior é aceitável, mas não 403 de origem.
    let (status, _, body) = send(
        &env,
        Method::POST,
        "/api/v1/uploads",
        &[
            ("cookie", admin.as_str()),
            ("host", "localhost"),
            ("origin", "http://localhost"),
        ],
        None,
    )
    .await;
    assert_ne!(
        status,
        StatusCode::FORBIDDEN,
        "mesma origem não deve ser bloqueada: {body}"
    );
}

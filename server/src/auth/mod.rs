//! Autenticação e autorização (M2a).
//!
//! Decisões (ADR-0010):
//! * senha com **Argon2id** (parâmetros padrão do crate `argon2`);
//! * sessão por **cookie HttpOnly** com token aleatório de 256 bits; o banco
//!   guarda apenas `sha256(token)`, nunca o token em claro;
//! * rotação de token em `/api/v1/auth/refresh` (revoga o anterior);
//! * três papéis fixos (`admin`, `moderator`, `user`) com permissões no código;
//! * mutações exigem mesma origem (`Origin`/`Sec-Fetch-Site`), mitigando CSRF
//!   sem token adicional;
//! * limite de tentativas de login em memória (por usuário normalizado).

pub mod middleware;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Nome do cookie de sessão.
pub const SESSION_COOKIE: &str = "zuno_session";
/// Validade padrão de uma sessão (30 dias).
pub const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// Validade do código de setup inicial (24 h).
pub const SETUP_TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// Validade padrão de um convite (7 dias).
pub const INVITATION_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Tentativas de login permitidas por janela.
pub const LOGIN_ATTEMPT_LIMIT: usize = 5;
/// Janela do limitador de tentativas.
pub const LOGIN_ATTEMPT_WINDOW: Duration = Duration::from_secs(5 * 60);

/// Papéis suportados (defaults restritivos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Moderator,
    User,
}

impl Role {
    /// Converte do valor persistido.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "admin" => Some(Self::Admin),
            "moderator" => Some(Self::Moderator),
            "user" => Some(Self::User),
            _ => None,
        }
    }

    /// Valor persistido em `users.role`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Moderator => "moderator",
            Self::User => "user",
        }
    }

    /// Administra usuários, convites e configuração da instância.
    pub fn is_admin(self) -> bool {
        self == Self::Admin
    }

    /// Pode editar metadados/capas e aprovar importações (M8).
    pub fn can_moderate(self) -> bool {
        matches!(self, Self::Admin | Self::Moderator)
    }
}

/// Usuário autenticado, injetado nas requisições pelo middleware.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentUser {
    pub id: String,
    pub username: String,
    pub role: Role,
    pub session_id: String,
}

/// Erros de autenticação.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// Credenciais inválidas (mensagem genérica: não revela se o usuário existe).
    #[error("usuário ou senha inválidos")]
    InvalidCredentials,
    /// Usuário desabilitado.
    #[error("usuário desabilitado")]
    Disabled,
    /// Sessão ausente, inválida ou expirada.
    #[error("sessão inválida")]
    InvalidSession,
    /// Limite de tentativas excedido.
    #[error("muitas tentativas; tente novamente em alguns minutos")]
    TooManyAttempts,
    /// Operação não permitida para o papel.
    #[error("permissão insuficiente")]
    Forbidden,
    /// Falha ao processar senha.
    #[error("falha ao processar senha")]
    Password,
}

/// Gera o hash Argon2id de uma senha.
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    if password.trim().len() < 8 {
        return Err(AuthError::Password);
    }

    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::Password)
}

/// Verifica uma senha contra o hash PHC persistido.
pub fn verify_password(password: &str, password_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(password_hash) else {
        return false;
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Gera um token opaco de 256 bits (dois UUIDv4 concatenados, sem hífens).
pub fn generate_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Hash do token, único valor persistido no banco.
pub fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

/// Gera um código de setup legível (8 caracteres, sem ambíguos).
pub fn generate_setup_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let token = generate_token();
    token
        .bytes()
        .take(8)
        .map(|byte| ALPHABET[(byte as usize) % ALPHABET.len()] as char)
        .collect()
}

/// Monta o `Set-Cookie` de sessão.
///
/// `secure` deve ser `true` quando a instância estiver atrás de HTTPS.
pub fn session_cookie(token: &str, secure: bool, max_age: Duration) -> String {
    let mut cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        max_age.as_secs()
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// Cookie que expira a sessão no navegador.
pub fn clear_session_cookie(secure: bool) -> String {
    let mut cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// Extrai o token de sessão do cabeçalho `Cookie`.
pub fn session_token_from_cookie(cookie_header: &str) -> Option<&str> {
    cookie_header.split(';').find_map(|pair| {
        let (name, value) = pair.trim().split_once('=')?;
        (name == SESSION_COOKIE).then_some(value)
    })
}

/// Limitador simples de tentativas de login (em memória, por processo).
#[derive(Debug, Default)]
pub struct LoginRateLimiter {
    attempts: Mutex<HashMap<String, Vec<Instant>>>,
}

impl LoginRateLimiter {
    /// Registra uma tentativa e devolve erro quando o limite foi excedido.
    pub fn check_and_record(&self, key: &str) -> Result<(), AuthError> {
        let now = Instant::now();
        let mut attempts = self
            .attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let entry = attempts.entry(key.to_string()).or_default();
        entry.retain(|instant| now.duration_since(*instant) < LOGIN_ATTEMPT_WINDOW);

        if entry.len() >= LOGIN_ATTEMPT_LIMIT {
            return Err(AuthError::TooManyAttempts);
        }

        entry.push(now);
        Ok(())
    }

    /// Limpa o histórico após login bem-sucedido.
    pub fn reset(&self, key: &str) {
        let mut attempts = self
            .attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        attempts.remove(key);
    }

    /// Chave do limitador: usuário normalizado + IP quando disponível.
    pub fn key(username: &str, ip: Option<IpAddr>) -> String {
        match ip {
            Some(ip) => format!("{username}@{ip}"),
            None => username.to_string(),
        }
    }
}

/// Verifica se uma mutação veio da mesma origem (mitigação de CSRF).
///
/// Requisições sem `Origin`/`Sec-Fetch-Site` (curl, testes, clientes nativos)
/// são aceitas: o cookie `SameSite=Lax` já bloqueia envio cross-site em navegadores.
pub fn is_same_origin(origin: Option<&str>, host: Option<&str>, fetch_site: Option<&str>) -> bool {
    if fetch_site == Some("cross-site") {
        return false;
    }

    let (Some(origin), Some(host)) = (origin, host) else {
        return true;
    };

    let Some((scheme, rest)) = origin.split_once("://") else {
        return false;
    };

    if scheme != "http" && scheme != "https" {
        return false;
    }

    rest.eq_ignore_ascii_case(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_round_trip() {
        let hash = hash_password("senha-muito-segura").expect("hash");
        assert!(hash.starts_with("$argon2"));
        assert!(verify_password("senha-muito-segura", &hash));
        assert!(!verify_password("senha-errada", &hash));
        assert!(!verify_password("senha-muito-segura", "hash-invalido"));
    }

    #[test]
    fn short_passwords_are_rejected() {
        assert!(matches!(hash_password("1234567"), Err(AuthError::Password)));
    }

    #[test]
    fn tokens_are_unique_and_hashed() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
        assert_eq!(hash_token(&a).len(), 64);
        assert_ne!(hash_token(&a), hash_token(&b));
    }

    #[test]
    fn setup_codes_are_short_and_unambiguous() {
        let code = generate_setup_code();
        assert_eq!(code.len(), 8);
        assert!(!code.contains('0') && !code.contains('O') && !code.contains('I'));
    }

    #[test]
    fn cookies_are_http_only_and_same_site() {
        let cookie = session_cookie("abc", true, SESSION_TTL);
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("Max-Age=2592000"));

        let cleared = clear_session_cookie(false);
        assert!(cleared.contains("Max-Age=0"));
        assert!(!cleared.contains("Secure"));
    }

    #[test]
    fn parses_session_cookie_header() {
        assert_eq!(
            session_token_from_cookie("outro=1; zuno_session=token123; x=2"),
            Some("token123")
        );
        assert_eq!(session_token_from_cookie("outro=1"), None);
    }

    #[test]
    fn rate_limiter_blocks_after_limit() {
        let limiter = LoginRateLimiter::default();
        for _ in 0..LOGIN_ATTEMPT_LIMIT {
            limiter.check_and_record("ana").expect("dentro do limite");
        }
        assert!(matches!(
            limiter.check_and_record("ana"),
            Err(AuthError::TooManyAttempts)
        ));
        limiter.reset("ana");
        limiter.check_and_record("ana").expect("reset libera");
    }

    #[test]
    fn same_origin_rules() {
        assert!(is_same_origin(None, Some("localhost:5173"), None));
        assert!(is_same_origin(
            Some("http://localhost:5173"),
            Some("localhost:5173"),
            Some("same-origin")
        ));
        assert!(!is_same_origin(
            Some("https://evil.example"),
            Some("localhost:5173"),
            None
        ));
        assert!(!is_same_origin(
            Some("http://localhost:5173"),
            Some("localhost:5173"),
            Some("cross-site")
        ));
        assert!(!is_same_origin(
            Some("javascript:alert(1)"),
            Some("localhost"),
            None
        ));
    }

    #[test]
    fn roles_parse_and_permissions() {
        assert_eq!(Role::parse("admin"), Some(Role::Admin));
        assert!(Role::Admin.is_admin());
        assert!(Role::Moderator.can_moderate());
        assert!(!Role::User.can_moderate());
        assert_eq!(Role::User.as_str(), "user");
    }
}

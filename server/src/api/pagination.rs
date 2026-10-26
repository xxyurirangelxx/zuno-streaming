//! Paginação por cursor opaco (keyset), sem dependências externas.
//!
//! O cursor codifica a chave de ordenação (`normalized_title` ou `name`) e o `id`,
//! em hexadecimal, no formato `chave_hex:id`. É opaco para o cliente e não expõe
//! SQL nem posições de offset (que degradam com o tamanho da biblioteca).

use crate::error::ApiError;

/// Limite padrão de itens por página.
pub const DEFAULT_LIMIT: i64 = 50;
/// Limite máximo aceito por página.
pub const MAX_LIMIT: i64 = 200;

/// Chave de paginação decodificada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub key: String,
    pub id: String,
}

impl Cursor {
    /// Serializa a chave + id em um token opaco.
    pub fn encode(key: &str, id: &str) -> String {
        format!("{}:{}", hex_encode(key.as_bytes()), id)
    }

    /// Decodifica um token; erro 400 `invalid_cursor` quando malformado.
    pub fn decode(token: &str) -> Result<Self, ApiError> {
        let Some((hex_key, id)) = token.split_once(':') else {
            return Err(ApiError::InvalidCursor);
        };

        if id.trim().is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(ApiError::InvalidCursor);
        }

        let key = hex_decode(hex_key).ok_or(ApiError::InvalidCursor)?;
        let key = String::from_utf8(key).map_err(|_| ApiError::InvalidCursor)?;

        Ok(Self {
            key,
            id: id.to_string(),
        })
    }
}

/// Normaliza o parâmetro `limit` (1..=MAX_LIMIT, padrão DEFAULT_LIMIT).
pub fn normalize_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }

    let mut bytes = Vec::with_capacity(value.len() / 2);
    let chars: Vec<char> = value.chars().collect();
    for pair in chars.chunks(2) {
        let high = pair[0].to_digit(16)?;
        let low = pair[1].to_digit(16)?;
        bytes.push(u8::try_from(high * 16 + low).ok()?);
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips_unicode_keys() {
        let token = Cursor::encode("canção № 1", "01a1b2c3-0000-7000-8000-000000000000");
        let decoded = Cursor::decode(&token).expect("cursor válido");
        assert_eq!(decoded.key, "canção № 1");
        assert_eq!(decoded.id, "01a1b2c3-0000-7000-8000-000000000000");
    }

    #[test]
    fn malformed_cursors_are_rejected() {
        for token in ["", "sem-separador", "zz:abc", "6162:", "61:nao valido"] {
            assert!(Cursor::decode(token).is_err(), "{token} deveria falhar");
        }
    }

    #[test]
    fn limit_is_clamped() {
        assert_eq!(normalize_limit(None), DEFAULT_LIMIT);
        assert_eq!(normalize_limit(Some(0)), 1);
        assert_eq!(normalize_limit(Some(10_000)), MAX_LIMIT);
    }
}

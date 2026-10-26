//! Streaming de áudio com `Range` e `If-Range` (RFC 9110 §14).
//!
//! Decisões desta entrega (M1b):
//! * biblioteca é somente Opus (ADR-0007): `Content-Type: audio/ogg`;
//! * intervalo único (`bytes=a-b`, `bytes=a-`, `bytes=-n`); múltiplos intervalos
//!   são atendidos com a representação completa (200), nunca com multipart;
//! * `ETag` = sha256 do arquivo; `If-Range` com ETag divergente devolve 200 completo;
//! * sem autenticação nesta fase (ADR-0008), bind loopback obrigatório;
//! * `HEAD` responde com os mesmos cabeçalhos e corpo vazio;
//! * arquivo inexistente/indisponível → 404; intervalo inválido → 416 + `Content-Range`.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::Method;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use super::{AppState, resolve_within};
use crate::error::ApiError;
use crate::repository;

/// Tamanho mínimo aceito para requisições `Range` (evita abuso trivial).
const MAX_RANGE_BYTES: u64 = 32 * 1024 * 1024;

/// `GET`/`HEAD /api/v1/tracks/{id}/stream`
pub async fn stream_track(
    State(state): State<AppState>,
    method: Method,
    Path(track_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let target = repository::get_stream_target(state.pool(), &track_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    if !target.available {
        tracing::debug!(track_id, "faixa sem arquivo disponível");
        return Err(ApiError::NotFound);
    }

    let root = state.config.music_dir.clone();
    let relative = target.relative_path.clone();
    let path = tokio::task::spawn_blocking(move || resolve_within(&root, &relative))
        .await
        .map_err(|error| {
            ApiError::Internal(anyhow::anyhow!("tarefa de resolução falhou: {error}"))
        })??;

    let mut file = tokio::fs::File::open(&path).await.map_err(|error| {
        tracing::warn!(error = %error, "arquivo indexado não pôde ser aberto");
        ApiError::NotFound
    })?;

    let metadata = file.metadata().await.map_err(|error| {
        tracing::warn!(error = %error, "falha ao ler metadados do arquivo");
        ApiError::NotFound
    })?;
    let size = metadata.len();
    let etag = format!("\"{}\"", target.sha256);

    if size == 0 {
        return Err(ApiError::RangeNotSatisfiable { resource_size: 0 });
    }

    let is_head = method == Method::HEAD;

    let range = if if_range_allows(&headers, &etag) {
        parse_range(
            headers
                .get(header::RANGE)
                .and_then(|value| value.to_str().ok()),
            size,
        )?
    } else {
        None
    };

    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("audio/ogg"));
    response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response_headers.insert(
        header::ETAG,
        HeaderValue::from_str(&etag)
            .map_err(|error| ApiError::Internal(anyhow::anyhow!("etag inválido: {error}")))?,
    );
    response_headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=0, must-revalidate"),
    );

    match range {
        Some((start, end)) => {
            let length = end - start + 1;
            file.seek(std::io::SeekFrom::Start(start))
                .await
                .map_err(|error| ApiError::Internal(anyhow::anyhow!("seek falhou: {error}")))?;

            response_headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end}/{size}")).map_err(|error| {
                    ApiError::Internal(anyhow::anyhow!("content-range: {error}"))
                })?,
            );
            response_headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&length.to_string()).map_err(|error| {
                    ApiError::Internal(anyhow::anyhow!("content-length: {error}"))
                })?,
            );

            if is_head {
                return Ok((StatusCode::PARTIAL_CONTENT, response_headers).into_response());
            }

            let reader = file.take(length);
            let body = Body::from_stream(ReaderStream::new(reader));

            Ok((StatusCode::PARTIAL_CONTENT, response_headers, body).into_response())
        }
        None => {
            response_headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&size.to_string()).map_err(|error| {
                    ApiError::Internal(anyhow::anyhow!("content-length: {error}"))
                })?,
            );

            if is_head {
                return Ok((StatusCode::OK, response_headers).into_response());
            }

            let body = Body::from_stream(ReaderStream::new(file));
            Ok((StatusCode::OK, response_headers, body).into_response())
        }
    }
}

fn if_range_allows(headers: &HeaderMap, etag: &str) -> bool {
    match headers
        .get(header::IF_RANGE)
        .and_then(|value| value.to_str().ok())
    {
        // Sem If-Range: o Range é aplicado normalmente.
        None => true,
        Some(value) => value.trim() == etag,
    }
}

/// Interpreta `Range: bytes=...` para um recurso de `size` bytes.
///
/// Retorna `Ok(None)` quando não há intervalo válido a aplicar (representação
/// completa), `Ok(Some((start, end)))` para um intervalo único e
/// `Err(RangeNotSatisfiable)` quando o intervalo é inválido/servível.
fn parse_range(header_value: Option<&str>, size: u64) -> Result<Option<(u64, u64)>, ApiError> {
    let Some(raw) = header_value else {
        return Ok(None);
    };

    let Some(spec) = raw.trim().strip_prefix("bytes=") else {
        return Ok(None);
    };

    // Múltiplos intervalos: servimos a representação completa (decisão documentada).
    if spec.contains(',') {
        return Ok(None);
    }

    let spec = spec.trim();
    let Some((start_raw, end_raw)) = spec.split_once('-') else {
        return Ok(None);
    };

    let unsat = || ApiError::RangeNotSatisfiable {
        resource_size: size,
    };

    let (start, end) = if start_raw.is_empty() {
        // bytes=-N → últimos N bytes.
        let suffix: u64 = end_raw.trim().parse().map_err(|_| unsat())?;
        if suffix == 0 {
            return Err(unsat());
        }
        let suffix = suffix.min(size);
        (size - suffix, size - 1)
    } else {
        let start: u64 = start_raw.trim().parse().map_err(|_| unsat())?;
        if start >= size {
            return Err(unsat());
        }
        let end = if end_raw.trim().is_empty() {
            size - 1
        } else {
            end_raw
                .trim()
                .parse::<u64>()
                .map_err(|_| unsat())?
                .min(size - 1)
        };

        if start > end {
            return Err(unsat());
        }
        (start, end)
    };

    if end - start + 1 > MAX_RANGE_BYTES {
        // Intervalo absurdo: devolve a representação completa em vez de 416.
        return Ok(None);
    }

    Ok(Some((start, end)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_ranges() {
        assert_eq!(
            parse_range(Some("bytes=0-499"), 1000).unwrap(),
            Some((0, 499))
        );
        assert_eq!(
            parse_range(Some("bytes=500-"), 1000).unwrap(),
            Some((500, 999))
        );
        assert_eq!(
            parse_range(Some("bytes=-200"), 1000).unwrap(),
            Some((800, 999))
        );
        assert_eq!(
            parse_range(Some("bytes=0-9999"), 1000).unwrap(),
            Some((0, 999))
        );
        assert_eq!(parse_range(None, 1000).unwrap(), None);
        assert_eq!(parse_range(Some("items=0-1"), 1000).unwrap(), None);
        assert_eq!(parse_range(Some("bytes=0-10,20-30"), 1000).unwrap(), None);
    }

    #[test]
    fn rejects_unsatisfiable_ranges() {
        assert!(matches!(
            parse_range(Some("bytes=1000-1200"), 1000),
            Err(ApiError::RangeNotSatisfiable { .. })
        ));
        assert!(matches!(
            parse_range(Some("bytes=abc-def"), 1000),
            Err(ApiError::RangeNotSatisfiable { .. })
        ));
        assert!(matches!(
            parse_range(Some("bytes=-0"), 1000),
            Err(ApiError::RangeNotSatisfiable { .. })
        ));
        assert!(matches!(
            parse_range(Some("bytes=500-100"), 1000),
            Err(ApiError::RangeNotSatisfiable { .. })
        ));
    }

    #[test]
    fn if_range_requires_matching_etag() {
        let mut headers = HeaderMap::new();
        assert!(if_range_allows(&headers, "\"abc\""));

        headers.insert(header::IF_RANGE, HeaderValue::from_static("\"abc\""));
        assert!(if_range_allows(&headers, "\"abc\""));

        headers.insert(header::IF_RANGE, HeaderValue::from_static("\"outro\""));
        assert!(!if_range_allows(&headers, "\"abc\""));
    }
}

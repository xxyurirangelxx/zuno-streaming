//! Upload de arquivos Opus com staging, validação e movimentação atômica (M1c).
//!
//! Regras (PRD §5.2 e ADR-0007):
//! * somente Opus é aceito; o resto é rejeitado com `415` e nunca convertido;
//! * o corpo é gravado em `ZUNO_STAGING_DIR` (fora da biblioteca) com nome
//!   derivado do UUID do upload — nunca do nome enviado pelo cliente;
//! * limite de tamanho aplicado durante a escrita;
//! * duplicata exata (`sha256` já indexado) responde `409` e **não** sobrescreve
//!   nem apaga o arquivo existente;
//! * a movimentação para a biblioteca é atômica (`rename` no mesmo volume);
//! * o arquivo é indexado imediatamente (sem esperar um scan completo).

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

use anyhow::{Context as _, anyhow, bail};
use axum::body::{Body, Bytes, HttpBody};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::config::Config;
use crate::library::metadata::{self, ExtractedMetadata, MetadataError};
use crate::repository::uploads::UploadRecord;

/// Staging mais antigo que isto é considerado órfão (best effort na inicialização).
const STAGING_ORPHAN_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const PART_EXTENSION: &str = "part";
const FINAL_EXTENSION: &str = "opus";
const HASH_CHUNK_BYTES: usize = 64 * 1024;
const MAX_NAME_COLLISION_ATTEMPTS: u32 = 10_000;

/// Estado de um upload persistido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadStatus {
    Staged,
    Indexed,
    Duplicate,
    Failed,
}

impl UploadStatus {
    /// Representação persistida em `uploads.status`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Staged => "staged",
            Self::Indexed => "indexed",
            Self::Duplicate => "duplicate",
            Self::Failed => "failed",
        }
    }
}

/// Falhas do fluxo de upload.
#[derive(Debug, Error)]
pub enum UploadError {
    /// Corpo maior que `ZUNO_MAX_AUDIO_FILE_BYTES`.
    #[error("arquivo excede o limite de {limit} bytes")]
    TooLarge { limit: u64 },
    /// Codec/contêiner diferente de Opus.
    #[error("formato não suportado: {0}")]
    UnsupportedFormat(String),
    /// Nome de arquivo ausente ou inválido.
    #[error("nome de arquivo ausente ou inválido")]
    InvalidFilename,
    /// Upload inexistente.
    #[error("upload não encontrado")]
    NotFound,
    /// Upload já finalizado (não está mais em staging).
    #[error("upload não está pendente")]
    NotPending,
    /// Arquivo Opus inválido/corrompido.
    #[error("arquivo Opus inválido: {0}")]
    InvalidAudio(String),
    /// Conteúdo já existe na biblioteca.
    #[error("arquivo já existe na biblioteca")]
    Duplicate { track_id: String },
    /// Falha de I/O.
    #[error("falha de I/O: {0}")]
    Io(#[from] io::Error),
    /// Falha interna (banco, configuração).
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// Resultado do staging de um arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedUpload {
    pub id: String,
    pub original_filename: String,
    pub size_bytes: u64,
    pub sha256: String,
}

/// Serviço de upload preso à configuração do servidor.
#[derive(Debug, Clone)]
pub struct UploadService {
    pub(crate) staging_dir: PathBuf,
    pub(crate) max_bytes: u64,
}

impl UploadService {
    /// Prepara o diretório de staging, recusando configurações perigosas.
    ///
    /// O staging nunca pode ficar dentro da biblioteca (o scanner acabaria
    /// indexando arquivos temporários). Arquivos órfãos com mais de 24 h são
    /// removidos na melhor das hipóteses, apenas com log.
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        fs::create_dir_all(&config.staging_dir).with_context(|| {
            format!(
                "não foi possível criar o diretório de staging: {}",
                config.staging_dir.display()
            )
        })?;

        let staging_dir = fs::canonicalize(&config.staging_dir).with_context(|| {
            format!(
                "diretório de staging inválido: {}",
                config.staging_dir.display()
            )
        })?;

        if !staging_dir.is_dir() {
            bail!("ZUNO_STAGING_DIR não é um diretório");
        }

        if is_within(&staging_dir, &config.music_dir).unwrap_or(false) {
            bail!(
                "ZUNO_STAGING_DIR não pode ficar dentro de ZUNO_MUSIC_DIR: \
                 o staging precisa estar fora da biblioteca"
            );
        }

        let service = Self {
            staging_dir,
            max_bytes: config.max_audio_file_bytes,
        };
        service.clean_orphans();

        Ok(service)
    }

    /// Caminho absoluto do arquivo final em staging (`{uuid}.opus`).
    pub fn finalized_path(&self, id: &str) -> PathBuf {
        self.staging_dir.join(format!("{id}.{FINAL_EXTENSION}"))
    }

    /// Resolve o arquivo de staging de um upload persistido, sem confiar no banco.
    pub fn staged_path(&self, record: &UploadRecord) -> Result<PathBuf, UploadError> {
        let file_name = Path::new(&record.staging_path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| UploadError::Internal(anyhow!("staging sem nome de arquivo")))?;

        if !file_name.starts_with(&record.id) {
            return Err(UploadError::Internal(anyhow!(
                "staging não corresponde ao upload"
            )));
        }

        let path = self.staging_dir.join(file_name);
        if !path.starts_with(&self.staging_dir) {
            return Err(UploadError::Internal(anyhow!("staging fora do diretório")));
        }

        Ok(path)
    }

    /// Remove somente o arquivo em staging (nunca a biblioteca).
    pub async fn remove_staging(&self, record: &UploadRecord) -> Result<(), UploadError> {
        let path = self.staged_path(record)?;

        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Grava o corpo em `{uuid}.part` em blocos, valida e renomeia para `.opus`.
    pub async fn stage(
        &self,
        filename: Option<&str>,
        body: Body,
    ) -> Result<StagedUpload, UploadError> {
        let original_filename = filename
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(UploadError::InvalidFilename)?;
        // O nome do cabeçalho é apenas metadado: nunca vira caminho.
        let original_filename = sanitize_component(original_filename, "upload.opus");

        let id = Uuid::now_v7().to_string();
        let part_path = self.staging_dir.join(format!("{id}.{PART_EXTENSION}"));

        match self
            .write_and_validate(&id, &original_filename, &part_path, body)
            .await
        {
            Ok(upload) => Ok(upload),
            Err(error) => {
                let _ = tokio::fs::remove_file(&part_path).await;
                Err(error)
            }
        }
    }

    async fn write_and_validate(
        &self,
        id: &str,
        original_filename: &str,
        part_path: &Path,
        body: Body,
    ) -> Result<StagedUpload, UploadError> {
        let mut file = tokio::fs::File::create_new(part_path).await?;
        let mut hasher = Sha256::new();
        let mut size_bytes = 0_u64;
        let mut body = Box::pin(body);

        loop {
            let next = std::future::poll_fn(|cx| poll_next_chunk(body.as_mut(), cx)).await;
            let Some(chunk) = next else {
                break;
            };
            let chunk = chunk.map_err(|error| {
                UploadError::Internal(anyhow!("falha ao ler o corpo do upload: {error}"))
            })?;

            size_bytes += chunk.len() as u64;
            if size_bytes > self.max_bytes {
                file.flush().await?;
                drop(file);
                return Err(UploadError::TooLarge {
                    limit: self.max_bytes,
                });
            }

            file.write_all(&chunk).await?;
            hasher.update(&chunk);
        }

        file.flush().await?;
        drop(file);

        let sha256 = format!("{:x}", hasher.finalize());
        let validation_path = part_path.to_path_buf();
        let extracted = tokio::task::spawn_blocking(move || metadata::extract(&validation_path))
            .await
            .map_err(|error| UploadError::Internal(anyhow!("tarefa de validação falhou: {error}")))?
            .map_err(|error| match error {
                MetadataError::Unsupported(kind) => UploadError::UnsupportedFormat(kind),
                MetadataError::Corrupt(message) => UploadError::InvalidAudio(message),
                MetadataError::Io(error) => UploadError::Io(error),
            })?;

        if !extracted.codec.eq_ignore_ascii_case("opus") {
            return Err(UploadError::UnsupportedFormat(extracted.codec));
        }

        let final_path = self.finalized_path(id);
        tokio::fs::rename(part_path, &final_path).await?;

        Ok(StagedUpload {
            id: id.to_string(),
            original_filename: original_filename.to_string(),
            size_bytes,
            sha256,
        })
    }

    /// Move o arquivo validado para a biblioteca, sem nunca sobrescrever nada.
    ///
    /// Devolve o caminho relativo final (com barras `/`) já indexável.
    pub async fn place_in_library(
        &self,
        config: &Config,
        staged_path: &Path,
        metadata: &ExtractedMetadata,
        sha256: &str,
        original_filename: &str,
    ) -> Result<String, UploadError> {
        let artist = first_non_empty(metadata.album_artist.as_deref())
            .or_else(|| {
                metadata
                    .artists
                    .iter()
                    .map(String::as_str)
                    .find(|name| !name.trim().is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "Artista desconhecido".to_string());

        let album =
            first_non_empty(metadata.album.as_deref()).unwrap_or_else(|| "Sem álbum".to_string());

        let title = first_non_empty(metadata.title.as_deref()).unwrap_or_else(|| {
            Path::new(original_filename)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| "Faixa sem título".to_string())
        });

        let relative =
            target_relative_path(&artist, &album, metadata.track_no, &title, FINAL_EXTENSION);

        let music_dir = config.music_dir.clone();
        let staged = staged_path.to_path_buf();
        let sha256 = sha256.to_string();

        tokio::task::spawn_blocking(move || {
            place_staged_file(&music_dir, &staged, &relative, &sha256)
        })
        .await
        .map_err(|error| UploadError::Internal(anyhow!("tarefa de movimentação falhou: {error}")))?
    }

    /// Remove arquivos de staging abandonados há mais de 24 h (best effort).
    fn clean_orphans(&self) {
        let Ok(entries) = fs::read_dir(&self.staging_dir) else {
            return;
        };

        let now = SystemTime::now();
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }

            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if extension != PART_EXTENSION && extension != FINAL_EXTENSION {
                continue;
            }

            let Ok(modified) = entry.metadata().and_then(|data| data.modified()) else {
                continue;
            };
            let Ok(age) = now.duration_since(modified) else {
                continue;
            };
            if age <= STAGING_ORPHAN_AGE {
                continue;
            }

            match fs::remove_file(&path) {
                Ok(()) => tracing::info!(
                    age_seconds = age.as_secs(),
                    "upload órfão removido do staging"
                ),
                Err(error) => tracing::warn!(
                    error = %error,
                    "falha ao remover upload órfão do staging"
                ),
            }
        }
    }
}

/// Lê o próximo bloco de dados do corpo, ignorando frames que não são dados.
fn poll_next_chunk(
    mut body: Pin<&mut Body>,
    cx: &mut Context<'_>,
) -> Poll<Option<Result<Bytes, axum::Error>>> {
    loop {
        match body.as_mut().poll_frame(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Ready(Some(Ok(frame))) => {
                if let Ok(data) = frame.into_data() {
                    return Poll::Ready(Some(Ok(data)));
                }
            }
            Poll::Ready(Some(Err(error))) => return Poll::Ready(Some(Err(error))),
        }
    }
}

fn place_staged_file(
    music_dir: &Path,
    staged: &Path,
    relative: &str,
    sha256: &str,
) -> Result<String, UploadError> {
    fs::create_dir_all(music_dir)?;
    let root = fs::canonicalize(music_dir)?;

    let mut candidate = relative.to_string();
    let mut suffix = 2_u32;

    loop {
        let target = root.join(&candidate);

        if !target.exists() {
            ensure_parent_within(&root, &target)?;
            move_file(staged, &target)?;
            return Ok(candidate);
        }

        if file_sha256(&target)? == sha256 {
            // O mesmo conteúdo já está no destino: não sobrescrevemos,
            // apenas descartamos a cópia em staging.
            if let Err(error) = fs::remove_file(staged) {
                tracing::warn!(error = %error, "falha ao remover staging duplicado");
            }
            return Ok(candidate);
        }

        if suffix > MAX_NAME_COLLISION_ATTEMPTS {
            return Err(UploadError::Internal(anyhow!(
                "não foi possível escolher um nome livre na biblioteca"
            )));
        }
        candidate = with_numeric_suffix(relative, suffix);
        suffix += 1;
    }
}

fn ensure_parent_within(root: &Path, target: &Path) -> Result<(), UploadError> {
    let parent = target
        .parent()
        .ok_or_else(|| UploadError::Internal(anyhow!("destino sem diretório pai")))?;

    fs::create_dir_all(parent)?;

    // Recusa diretórios que resolvem (via symlink) para fora da biblioteca.
    let canonical_parent = fs::canonicalize(parent)?;
    if !canonical_parent.starts_with(root) {
        return Err(UploadError::Internal(anyhow!(
            "destino resolve para fora da biblioteca"
        )));
    }

    Ok(())
}

fn move_file(staged: &Path, target: &Path) -> Result<(), UploadError> {
    match fs::rename(staged, target) {
        Ok(()) => Ok(()),
        Err(error) if is_cross_device(&error) => {
            // Volumes diferentes: copia para um `.tmp` no destino final e
            // renomeia (evita destino parcial) e só então remove o staging.
            let file_name = target
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("upload.opus");
            let temporary = target.with_file_name(format!(".{file_name}.tmp"));

            if let Err(error) = fs::copy(staged, &temporary) {
                let _ = fs::remove_file(&temporary);
                return Err(error.into());
            }

            if let Err(error) = fs::rename(&temporary, target) {
                let _ = fs::remove_file(&temporary);
                return Err(error.into());
            }

            if let Err(error) = fs::remove_file(staged) {
                tracing::warn!(
                    error = %error,
                    "staging não removido depois da cópia entre volumes"
                );
            }

            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn is_cross_device(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::CrossesDevices {
        return true;
    }

    // EXDEV (unix) / ERROR_NOT_SAME_DEVICE (windows) em versões sem o kind.
    matches!(error.raw_os_error(), Some(18) | Some(17))
}

fn file_sha256(path: &Path) -> Result<String, UploadError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_CHUNK_BYTES];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn with_numeric_suffix(relative: &str, suffix: u32) -> String {
    let (directory, file_name) = match relative.rsplit_once('/') {
        Some((directory, file_name)) => (directory, file_name),
        None => ("", relative),
    };

    let (stem, extension) = match file_name.rsplit_once('.') {
        Some((stem, extension)) => (stem, extension),
        None => (file_name, ""),
    };

    let new_name = if extension.is_empty() {
        format!("{stem} ({suffix})")
    } else {
        format!("{stem} ({suffix}).{extension}")
    };

    if directory.is_empty() {
        new_name
    } else {
        format!("{directory}/{new_name}")
    }
}

fn first_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn is_within(path: &Path, root: &Path) -> io::Result<bool> {
    let path = fs::canonicalize(path)?;
    let root = match fs::canonicalize(root) {
        Ok(root) => root,
        Err(error) if error.kind() == io::ErrorKind::NotFound => std::path::absolute(root)?,
        Err(error) => return Err(error),
    };

    Ok(path.starts_with(root))
}

/// Caminho relativo de destino na biblioteca, derivado dos metadados.
pub fn target_relative_path(
    artist: &str,
    album: &str,
    track_no: Option<u32>,
    title: &str,
    extension: &str,
) -> String {
    let artist = sanitize_component(artist, "Artista desconhecido");
    let album = sanitize_component(album, "Sem álbum");
    let title = sanitize_component(title, "Faixa sem título");

    let file_name = match track_no {
        Some(number) => format!("{number:02} {title}.{extension}"),
        None => format!("{title}.{extension}"),
    };

    format!("{artist}/{album}/{file_name}")
}

/// Remove caracteres perigosos de um componente de caminho.
pub fn sanitize_component(value: &str, fallback: &str) -> String {
    let cleaned: String = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();

    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim_matches('.').trim();

    if cleaned.is_empty() {
        return fallback.to_string();
    }

    cleaned.chars().take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(staging_dir: PathBuf, music_dir: PathBuf) -> Config {
        Config {
            bind_addr: "127.0.0.1:0".parse().expect("bind"),
            database_url: "sqlite::memory:".to_string(),
            music_dir,
            artwork_dir: PathBuf::from("artwork"),
            staging_dir,
            max_audio_file_bytes: 1024 * 1024,
            scan_on_startup: false,
            db_max_connections: 1,
        }
    }

    #[test]
    fn sanitizes_path_components() {
        assert_eq!(sanitize_component("  AC/DC  ", "x"), "AC_DC");
        // Separadores viram `_`; os pontos restantes são inofensivos porque o
        // componente nunca é interpretado como caminho relativo.
        assert_eq!(sanitize_component("..\\..\\evil", "x"), "_.._evil");
        assert_eq!(sanitize_component("   ", "fallback"), "fallback");
        assert_eq!(sanitize_component(".hidden.", "x"), "hidden");
    }

    #[test]
    fn builds_target_paths_without_escaping_the_library() {
        let path = target_relative_path("AC/DC", "Back in Black", Some(1), "Hells Bells", "opus");
        assert_eq!(path, "AC_DC/Back in Black/01 Hells Bells.opus");
        assert!(!path.contains(".."));
    }

    #[test]
    fn numeric_suffix_is_inserted_before_the_extension() {
        assert_eq!(
            with_numeric_suffix("Artist/Album/01 Song.opus", 2),
            "Artist/Album/01 Song (2).opus"
        );
        assert_eq!(with_numeric_suffix("song.opus", 3), "song (3).opus");
    }

    #[test]
    fn staging_inside_the_library_is_rejected() {
        let dir = tempfile::tempdir().expect("diretório temporário");
        let music_dir = dir.path().join("music");
        std::fs::create_dir_all(&music_dir).expect("criar música");

        let config = config_with(music_dir.join("staging"), music_dir);
        let error = UploadService::new(&config).expect_err("staging dentro da biblioteca");

        assert!(format!("{error:#}").contains("ZUNO_STAGING_DIR"));
    }

    #[test]
    fn staging_outside_the_library_is_accepted() {
        let dir = tempfile::tempdir().expect("diretório temporário");
        let music_dir = dir.path().join("music");
        std::fs::create_dir_all(&music_dir).expect("criar música");

        let config = config_with(dir.path().join("staging"), music_dir);
        let service = UploadService::new(&config).expect("staging válido");

        assert!(service.staging_dir.is_dir());
    }
}

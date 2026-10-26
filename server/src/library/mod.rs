//! Varredura da biblioteca musical — somente leitura, somente Opus (ADR-0007).
//!
//! Garantias:
//! * nunca move, renomeia ou apaga arquivos do usuário;
//! * ignora links simbólicos e caminhos que escapam da raiz;
//! * arquivo ausente vira `available = 0` (a linha nunca é apagada);
//! * reexecutar o scan é idempotente (reaproveita faixa/arquivo pelo caminho);
//! * problemas são registrados em `scan_issues` e devolvidos no relatório.

pub mod metadata;

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::config::Config;
use crate::db::Database;
use crate::domain::{
    AudioFileDraft, ScanIssue, ScanIssueCode, ScanReport, TrackDraft, normalize_key,
};
use crate::library::metadata::{ExtractedMetadata, MetadataError};
use crate::repository::{self, ArtworkDraft, AudioFileState, ScanCounts};

/// Extensões consideradas no scan. O codec é confirmado pela leitura dos metadados.
const AUDIO_EXTENSIONS: [&str; 2] = ["opus", "ogg"];
const HASH_CHUNK_BYTES: usize = 64 * 1024;

/// Opções de uma varredura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanOptions {
    /// Tamanho máximo por arquivo; acima disso o arquivo é reportado e ignorado.
    pub max_file_bytes: u64,
    /// Extrai capas embutidas para o cache local.
    pub extract_artwork: bool,
}

impl ScanOptions {
    /// Opções derivadas da configuração do servidor.
    pub fn from_config(config: &Config) -> Self {
        Self {
            max_file_bytes: config.max_audio_file_bytes,
            extract_artwork: true,
        }
    }
}

/// Varredor da biblioteca, preso a uma raiz canônica.
#[derive(Debug, Clone)]
pub struct LibraryScanner {
    root: PathBuf,
    artwork_dir: PathBuf,
    options: ScanOptions,
}

#[derive(Debug)]
struct Candidate {
    relative_path: String,
    absolute: PathBuf,
    size_bytes: u64,
    source_mtime: i64,
}

#[derive(Debug)]
struct Processed {
    candidate: Candidate,
    sha256: String,
    metadata: ExtractedMetadata,
    artwork: Option<CachedArtwork>,
}

#[derive(Debug)]
struct CachedArtwork {
    relative_path: String,
    mime_type: Option<String>,
    checksum: String,
}

impl LibraryScanner {
    /// Prepara o scanner; exige que o diretório da biblioteca exista.
    pub fn new(config: &Config, options: ScanOptions) -> Result<Self> {
        let root = fs::canonicalize(&config.music_dir).with_context(|| {
            format!(
                "diretório da biblioteca não encontrado: {} (crie o diretório ou ajuste ZUNO_MUSIC_DIR)",
                config.music_dir.display()
            )
        })?;

        if !root.is_dir() {
            bail!("ZUNO_MUSIC_DIR não é um diretório: {}", root.display());
        }

        fs::create_dir_all(&config.artwork_dir).with_context(|| {
            format!(
                "não foi possível criar o diretório de capas: {}",
                config.artwork_dir.display()
            )
        })?;
        let artwork_dir = fs::canonicalize(&config.artwork_dir).with_context(|| {
            format!(
                "diretório de capas inválido: {}",
                config.artwork_dir.display()
            )
        })?;

        Ok(Self {
            root,
            artwork_dir,
            options,
        })
    }

    /// Executa uma varredura completa e persiste o resultado.
    pub async fn scan(&self, db: &Database) -> Result<ScanReport> {
        let scan_run_id = Uuid::now_v7().to_string();
        repository::start_scan_run(db.pool(), &scan_run_id).await?;

        match self.run_scan(db, &scan_run_id).await {
            Ok(report) => {
                repository::finish_scan_run(
                    db.pool(),
                    &scan_run_id,
                    ScanCounts::from(&report),
                    None,
                )
                .await?;
                Ok(report)
            }
            Err(error) => {
                let message = format!("{error:#}");
                if let Err(finish_error) = repository::finish_scan_run(
                    db.pool(),
                    &scan_run_id,
                    ScanCounts::default(),
                    Some(&message),
                )
                .await
                {
                    tracing::warn!(error = %finish_error, "falha ao registrar o fim do scan");
                }
                Err(error)
            }
        }
    }

    async fn run_scan(&self, db: &Database, scan_run_id: &str) -> Result<ScanReport> {
        let root = self.root.clone();
        let artwork_dir = self.artwork_dir.clone();
        let max_file_bytes = self.options.max_file_bytes;
        let (candidates, mut issues, files_seen) = tokio::task::spawn_blocking(move || {
            collect_candidates(&root, &artwork_dir, max_file_bytes)
        })
        .await
        .context("tarefa de varredura falhou")??;

        let states = repository::load_audio_file_states(db.pool()).await?;
        let mut seen_paths = Vec::with_capacity(candidates.len());
        let mut pending = Vec::new();
        let mut unchanged = 0_u64;

        for candidate in candidates {
            seen_paths.push(candidate.relative_path.clone());
            let is_unchanged = states
                .get(&candidate.relative_path)
                .is_some_and(|state| state.is_fresh(&candidate));
            if is_unchanged {
                unchanged += 1;
            } else {
                pending.push(candidate);
            }
        }

        let extract_artwork = self.options.extract_artwork;
        let artwork_dir = self.artwork_dir.clone();
        let (processed, processing_issues) = tokio::task::spawn_blocking(move || {
            process_files(pending, &artwork_dir, extract_artwork)
        })
        .await
        .context("tarefa de extração de metadados falhou")?;
        issues.extend(processing_issues);

        let mut indexed = 0_u64;
        for file in processed {
            if let Some(issue) = self.persist_file(db, &states, file).await? {
                issues.push(issue);
            }
            indexed += 1;
        }

        let files_missing = repository::mark_missing_audio_files(db.pool(), &seen_paths).await?;

        for issue in &issues {
            repository::record_scan_issue(db.pool(), scan_run_id, issue).await?;
        }

        let (skipped, failed) = issues
            .iter()
            .fold((0_u64, 0_u64), |(skipped, failed), issue| {
                match issue.code {
                    ScanIssueCode::CorruptFile | ScanIssueCode::ReadError => (skipped, failed + 1),
                    // A duplicata exata continua indexada; não conta como ignorada.
                    ScanIssueCode::DuplicateExact => (skipped, failed),
                    _ => (skipped + 1, failed),
                }
            });

        let report = ScanReport {
            scan_run_id: scan_run_id.to_string(),
            files_seen,
            files_indexed: indexed,
            files_unchanged: unchanged,
            files_skipped: skipped,
            files_failed: failed,
            files_missing,
            issues,
        };

        tracing::info!(
            seen = report.files_seen,
            indexed = report.files_indexed,
            unchanged = report.files_unchanged,
            skipped = report.files_skipped,
            failed = report.files_failed,
            missing = report.files_missing,
            "varredura da biblioteca concluída"
        );

        Ok(report)
    }

    async fn persist_file(
        &self,
        db: &Database,
        states: &HashMap<String, AudioFileState>,
        file: Processed,
    ) -> Result<Option<ScanIssue>> {
        let existing = states.get(&file.candidate.relative_path);
        let track_id = existing
            .map(|state| state.track_id.clone())
            .unwrap_or_else(|| Uuid::now_v7().to_string());
        let audio_file_id = existing
            .map(|state| state.id.clone())
            .unwrap_or_else(|| Uuid::now_v7().to_string());

        let metadata = &file.metadata;

        let mut artist_ids = Vec::with_capacity(metadata.artists.len());
        for name in &metadata.artists {
            artist_ids.push(repository::upsert_artist(db.pool(), name).await?);
        }

        let album_artist_source = metadata
            .album_artist
            .clone()
            .or_else(|| metadata.artists.first().cloned());
        let album_artist_id = match album_artist_source {
            Some(name) => Some(repository::upsert_artist(db.pool(), &name).await?),
            None => None,
        };

        let album_id = match metadata.album.as_deref() {
            Some(title) => Some(
                repository::upsert_album(
                    db.pool(),
                    title,
                    album_artist_id.as_deref(),
                    metadata.release_year,
                    None,
                )
                .await?,
            ),
            None => None,
        };

        let fallback_title = file
            .candidate
            .absolute
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Faixa sem título".to_string());
        let title = metadata
            .title
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(fallback_title);

        let track = TrackDraft {
            id: track_id.clone(),
            album_id,
            normalized_title: normalize_key(&title),
            title,
            disc_no: metadata.disc_no,
            track_no: metadata.track_no,
            duration_ms: metadata.duration_ms,
            artists: artist_ids
                .iter()
                .enumerate()
                .map(|(index, id)| (id.clone(), "performer".to_string(), index as u32))
                .collect(),
        };

        let audio_file = AudioFileDraft {
            id: audio_file_id.clone(),
            track_id,
            relative_path: file.candidate.relative_path.clone(),
            sha256: file.sha256.clone(),
            codec: metadata.codec.clone(),
            container: metadata.container.clone(),
            bitrate: metadata.bitrate_kbps,
            sample_rate: metadata.sample_rate_hz,
            channels: metadata.channels,
            size_bytes: file.candidate.size_bytes,
            source_mtime: file.candidate.source_mtime,
            available: true,
        };

        let duplicate = repository::find_audio_file_by_sha(db.pool(), &file.sha256).await?;
        let duplicate_issue = duplicate
            .filter(|id| id != &audio_file_id)
            .map(|_| ScanIssue {
                relative_path: file.candidate.relative_path.clone(),
                code: ScanIssueCode::DuplicateExact,
                detail: Some(
                    "mesmo sha256 de outro arquivo já indexado; mantido, sem exclusão".to_string(),
                ),
            });

        let artwork = file.artwork.as_ref().map(|artwork| ArtworkDraft {
            relative_path: artwork.relative_path.clone(),
            source: "embedded".to_string(),
            mime_type: artwork.mime_type.clone(),
            checksum: artwork.checksum.clone(),
        });

        repository::persist_track_with_file(db.pool(), &track, &audio_file, artwork.as_ref())
            .await?;

        Ok(duplicate_issue)
    }
}

impl AudioFileState {
    fn is_fresh(&self, candidate: &Candidate) -> bool {
        self.available
            && self.size_bytes == candidate.size_bytes
            && self.source_mtime == candidate.source_mtime
    }
}

fn collect_candidates(
    root: &Path,
    artwork_dir: &Path,
    max_file_bytes: u64,
) -> Result<(Vec<Candidate>, Vec<ScanIssue>, u64)> {
    let mut candidates = Vec::new();
    let mut issues = Vec::new();
    let mut files_seen = 0_u64;

    for entry in WalkDir::new(root).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                issues.push(ScanIssue {
                    relative_path: error
                        .path()
                        .map(|path| relative_path(root, path))
                        .unwrap_or_default(),
                    code: ScanIssueCode::ReadError,
                    detail: Some(error.to_string()),
                });
                continue;
            }
        };

        let path = entry.path();
        if path == root || entry.file_type().is_dir() {
            continue;
        }

        let relative = relative_path(root, path);

        if entry.file_type().is_symlink() {
            issues.push(ScanIssue {
                relative_path: relative,
                code: ScanIssueCode::SymlinkSkipped,
                detail: Some("links simbólicos não são seguidos".to_string()),
            });
            continue;
        }

        if path.starts_with(artwork_dir) {
            continue;
        }

        let absolute = match fs::canonicalize(path) {
            Ok(path) => path,
            Err(error) => {
                issues.push(ScanIssue {
                    relative_path: relative,
                    code: ScanIssueCode::ReadError,
                    detail: Some(error.to_string()),
                });
                continue;
            }
        };

        if !absolute.starts_with(root) {
            issues.push(ScanIssue {
                relative_path: relative,
                code: ScanIssueCode::OutsideRoot,
                detail: Some("o caminho resolve para fora da biblioteca".to_string()),
            });
            continue;
        }

        files_seen += 1;

        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if !AUDIO_EXTENSIONS.contains(&extension.as_str()) {
            issues.push(ScanIssue {
                relative_path: relative,
                code: ScanIssueCode::UnsupportedFormat,
                detail: Some(format!(
                    "extensão '.{extension}' ignorada; converta para Opus no cliente (ADR-0007)"
                )),
            });
            continue;
        }

        let metadata = match fs::metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(error) => {
                issues.push(ScanIssue {
                    relative_path: relative,
                    code: ScanIssueCode::ReadError,
                    detail: Some(error.to_string()),
                });
                continue;
            }
        };

        if metadata.len() > max_file_bytes {
            issues.push(ScanIssue {
                relative_path: relative,
                code: ScanIssueCode::TooLarge,
                detail: Some(format!(
                    "{} bytes excede o limite de {max_file_bytes} bytes",
                    metadata.len()
                )),
            });
            continue;
        }

        let source_mtime = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();

        candidates.push(Candidate {
            relative_path: relative,
            absolute,
            size_bytes: metadata.len(),
            source_mtime,
        });
    }

    Ok((candidates, issues, files_seen))
}

fn process_files(
    candidates: Vec<Candidate>,
    artwork_dir: &Path,
    extract_artwork: bool,
) -> (Vec<Processed>, Vec<ScanIssue>) {
    let mut processed = Vec::with_capacity(candidates.len());
    let mut issues = Vec::new();

    for candidate in candidates {
        let metadata = match metadata::extract(&candidate.absolute) {
            Ok(metadata) => metadata,
            Err(MetadataError::Unsupported(kind)) => {
                issues.push(ScanIssue {
                    relative_path: candidate.relative_path,
                    code: ScanIssueCode::UnsupportedFormat,
                    detail: Some(format!("{kind}; converta para Opus no cliente (ADR-0007)")),
                });
                continue;
            }
            Err(MetadataError::Corrupt(message)) => {
                issues.push(ScanIssue {
                    relative_path: candidate.relative_path,
                    code: ScanIssueCode::CorruptFile,
                    detail: Some(message),
                });
                continue;
            }
            Err(MetadataError::Io(error)) => {
                issues.push(ScanIssue {
                    relative_path: candidate.relative_path,
                    code: ScanIssueCode::ReadError,
                    detail: Some(error.to_string()),
                });
                continue;
            }
        };

        if !metadata.codec.eq_ignore_ascii_case("opus") {
            issues.push(ScanIssue {
                relative_path: candidate.relative_path,
                code: ScanIssueCode::UnsupportedFormat,
                detail: Some(format!(
                    "codec '{}' não suportado; a biblioteca é somente Opus",
                    metadata.codec
                )),
            });
            continue;
        }

        let sha256 = match hash_file(&candidate.absolute) {
            Ok(sha256) => sha256,
            Err(error) => {
                issues.push(ScanIssue {
                    relative_path: candidate.relative_path,
                    code: ScanIssueCode::ReadError,
                    detail: Some(error.to_string()),
                });
                continue;
            }
        };

        let artwork = if extract_artwork {
            metadata.artwork.as_ref().and_then(|artwork| {
                match cache_artwork(artwork_dir, &artwork.bytes, artwork.mime_type.as_deref()) {
                    Ok(cached) => Some(cached),
                    Err(error) => {
                        issues.push(ScanIssue {
                            relative_path: candidate.relative_path.clone(),
                            code: ScanIssueCode::ReadError,
                            detail: Some(format!("falha ao gravar capa: {error:#}")),
                        });
                        None
                    }
                }
            })
        } else {
            None
        };

        processed.push(Processed {
            candidate,
            sha256,
            metadata,
            artwork,
        });
    }

    (processed, issues)
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("não foi possível abrir {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_CHUNK_BYTES];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("falha ao ler {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn cache_artwork(
    artwork_dir: &Path,
    bytes: &[u8],
    mime_type: Option<&str>,
) -> Result<CachedArtwork> {
    let checksum = format!("{:x}", Sha256::digest(bytes));
    let extension = artwork_extension(bytes, mime_type);
    let file_name = format!("{checksum}.{extension}");
    let target = artwork_dir.join(&file_name);

    if !target.exists() {
        let temporary = artwork_dir.join(format!(".{file_name}.tmp"));
        fs::write(&temporary, bytes)
            .with_context(|| format!("falha ao gravar capa temporária {}", temporary.display()))?;
        fs::rename(&temporary, &target)
            .with_context(|| format!("falha ao mover capa para {}", target.display()))?;
    }

    Ok(CachedArtwork {
        relative_path: file_name,
        mime_type: mime_type.map(str::to_string),
        checksum,
    })
}

fn artwork_extension(bytes: &[u8], mime_type: Option<&str>) -> &'static str {
    if let Some(mime) = mime_type {
        match mime.to_ascii_lowercase().as_str() {
            "image/jpeg" | "image/jpg" => return "jpg",
            "image/png" => return "png",
            "image/webp" => return "webp",
            "image/gif" => return "gif",
            _ => {}
        }
    }

    match infer::get(bytes).map(|kind| kind.mime_type()) {
        Some("image/jpeg") => "jpg",
        Some("image/png") => "png",
        Some("image/webp") => "webp",
        Some("image/gif") => "gif",
        _ => "bin",
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artwork_extension_prefers_mime_then_magic() {
        let png_magic = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        assert_eq!(artwork_extension(&png_magic, None), "png");
        assert_eq!(artwork_extension(&png_magic, Some("image/jpeg")), "jpg");
        assert_eq!(artwork_extension(b"nope", None), "bin");
    }

    #[test]
    fn relative_path_uses_forward_slashes() {
        let root = Path::new("/music");
        assert_eq!(
            relative_path(root, Path::new("/music/Artist/Album/track.opus")),
            "Artist/Album/track.opus"
        );
    }
}

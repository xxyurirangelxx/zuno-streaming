//! Tipos do domínio musical.
//!
//! A camada de domínio não conhece Axum nem SQLx: são structs/enums com os dados
//! que atravessam o scanner, o repositório e (no M1b) a API.

use std::fmt;

use serde::{Deserialize, Serialize};

pub mod dto;

/// Normaliza texto para comparação (trim, minúsculas, espaços colapsados).
///
/// Não remove acentos: a busca sem acento é responsabilidade do tokenizer do FTS5.
pub fn normalize_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Artista (nome canônico, sem variantes de grafia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

/// Álbum ao qual faixas pertencem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub album_artist_id: Option<String>,
    pub release_year: Option<i32>,
    pub artwork_id: Option<String>,
}

/// Faixa lógica (metadados), independente do arquivo físico.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub album_id: Option<String>,
    pub title: String,
    pub disc_no: Option<u32>,
    pub track_no: Option<u32>,
    pub duration_ms: Option<u64>,
    pub artists: Vec<TrackArtist>,
}

/// Vínculo entre faixa e artista, com papel e ordenação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackArtist {
    pub artist_id: String,
    pub role: String,
    pub sort_order: u32,
}

/// Arquivo de áudio indexado. Caminho é atributo mutável, nunca identidade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFile {
    pub id: String,
    pub track_id: String,
    pub relative_path: String,
    pub sha256: String,
    pub codec: String,
    pub container: String,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub size_bytes: u64,
    pub available: bool,
}

/// Capa extraída para o cache local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artwork {
    pub id: String,
    pub relative_path: String,
    pub source: ArtworkSource,
    pub mime_type: Option<String>,
    pub checksum: String,
}

/// Origem da capa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkSource {
    Embedded,
    Folder,
    Manual,
}

impl ArtworkSource {
    /// Representação persistida em `artworks.source`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::Folder => "folder",
            Self::Manual => "manual",
        }
    }
}

/// Códigos estáveis de problema encontrado durante o scan.
///
/// Nunca apagamos nem movemos arquivos do usuário: problemas são registrados
/// em `scan_issues` e devolvidos no relatório.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanIssueCode {
    /// Formato diferente de Opus (ADR-0007): ignorado, nunca convertido.
    UnsupportedFormat,
    /// Arquivo Opus ilegível/corrompido.
    CorruptFile,
    /// Arquivo maior que o limite configurado.
    TooLarge,
    /// Link simbólico ignorado por segurança.
    SymlinkSkipped,
    /// Caminho que resolve para fora da raiz da biblioteca.
    OutsideRoot,
    /// Mesmo sha256 de outro arquivo já indexado.
    DuplicateExact,
    /// Falha de I/O ao ler o arquivo.
    ReadError,
}

impl ScanIssueCode {
    /// Representação persistida em `scan_issues.code`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedFormat => "unsupported_format",
            Self::CorruptFile => "corrupt_file",
            Self::TooLarge => "too_large",
            Self::SymlinkSkipped => "symlink_skipped",
            Self::OutsideRoot => "outside_root",
            Self::DuplicateExact => "duplicate_exact",
            Self::ReadError => "read_error",
        }
    }
}

impl fmt::Display for ScanIssueCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Problema isolado de um arquivo durante o scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanIssue {
    pub relative_path: String,
    pub code: ScanIssueCode,
    pub detail: Option<String>,
}

/// Resultado de uma varredura da biblioteca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanReport {
    pub scan_run_id: String,
    pub files_seen: u64,
    pub files_indexed: u64,
    pub files_unchanged: u64,
    pub files_skipped: u64,
    pub files_failed: u64,
    pub files_missing: u64,
    pub issues: Vec<ScanIssue>,
}

/// Faixa pronta para persistência (draft do repositório).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackDraft {
    pub id: String,
    pub album_id: Option<String>,
    pub title: String,
    pub normalized_title: String,
    pub disc_no: Option<u32>,
    pub track_no: Option<u32>,
    pub duration_ms: Option<u64>,
    /// `(artist_id, role, sort_order)`
    pub artists: Vec<(String, String, u32)>,
}

/// Arquivo de áudio pronto para persistência (draft do repositório).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFileDraft {
    pub id: String,
    pub track_id: String,
    pub relative_path: String,
    pub sha256: String,
    pub codec: String,
    pub container: String,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub size_bytes: u64,
    pub source_mtime: i64,
    pub available: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_key_trims_case_and_spaces() {
        assert_eq!(normalize_key("  Daft   Punk "), "daft punk");
        assert_eq!(normalize_key("ÁÉÍ"), "áéí");
        assert_eq!(normalize_key(""), "");
    }

    #[test]
    fn issue_codes_are_stable_snake_case() {
        assert_eq!(
            ScanIssueCode::UnsupportedFormat.as_str(),
            "unsupported_format"
        );
        assert_eq!(ScanIssueCode::DuplicateExact.as_str(), "duplicate_exact");
        assert_eq!(ArtworkSource::Embedded.as_str(), "embedded");
    }
}

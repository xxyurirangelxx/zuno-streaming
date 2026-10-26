//! Extração de metadados e detecção de formato de áudio.
//!
//! A biblioteca é somente Opus (ADR-0007). Este módulo apenas **lê** o arquivo:
//! nunca escreve tags, nunca move e nunca apaga nada.

use std::fs::File;
use std::io::{self, ErrorKind, Read};
use std::path::Path;

use lofty::error::FileParseError;
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::prelude::Accessor;
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag};
use thiserror::Error;

/// Quantidade de bytes lida do início do arquivo para identificar o formato.
const HEADER_BYTES: usize = 36;

/// Capa embutida no arquivo, em bytes brutos (o cache/resize é responsabilidade
/// da camada de biblioteca).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedArtwork {
    pub bytes: Vec<u8>,
    pub mime_type: Option<String>,
}

/// Metadados extraídos de um arquivo Opus.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExtractedMetadata {
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub release_year: Option<i32>,
    pub duration_ms: Option<u64>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
    /// Codec detectado (deve ser `opus` para arquivos aceitos).
    pub codec: String,
    /// Contêiner detectado (`ogg` para Opus em Ogg).
    pub container: String,
    pub artwork: Option<ExtractedArtwork>,
}

/// Falhas possíveis ao ler um arquivo de áudio.
#[derive(Debug, Error)]
pub enum MetadataError {
    /// Áudio válido, mas de codec/contêiner diferente de Opus.
    #[error("formato não suportado: {0}")]
    Unsupported(String),
    /// Arquivo ilegível/corrompido para o parser.
    #[error("arquivo de áudio inválido: {0}")]
    Corrupt(String),
    /// Falha de I/O.
    #[error("falha de I/O: {0}")]
    Io(#[from] std::io::Error),
}

/// Extrai metadados de um arquivo Opus.
///
/// Retorna `Unsupported` para áudio válido que não seja Opus e `Corrupt` quando
/// o parser não consegue ler o arquivo.
pub fn extract(path: &Path) -> Result<ExtractedMetadata, MetadataError> {
    let header = read_header(path)?;
    let Some(file_type) = FileType::from_buffer(&header) else {
        return Err(MetadataError::Corrupt(
            "conteúdo não reconhecido como áudio (magic bytes desconhecidos)".to_string(),
        ));
    };

    if file_type != FileType::Opus {
        return Err(MetadataError::Unsupported(file_type_name(file_type)));
    }

    // O tipo é fixado a partir do conteúdo, nunca da extensão do arquivo.
    let tagged_file = Probe::open(path)
        .map_err(parse_error)?
        .set_file_type(FileType::Opus)
        .read()
        .map_err(parse_error)?;

    if tagged_file.file_type() != FileType::Opus {
        return Err(MetadataError::Unsupported(file_type_name(
            tagged_file.file_type(),
        )));
    }

    let properties = tagged_file.properties();
    let duration_ms = {
        let duration = properties.duration();
        if duration.is_zero() {
            None
        } else {
            Some(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        }
    };

    let mut metadata = ExtractedMetadata {
        duration_ms,
        bitrate_kbps: properties.audio_bitrate().filter(|value| *value > 0),
        sample_rate_hz: properties.sample_rate().filter(|value| *value > 0),
        channels: properties.channels().filter(|value| *value > 0),
        codec: "opus".to_string(),
        container: "ogg".to_string(),
        ..ExtractedMetadata::default()
    };

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());
    if let Some(tag) = tag {
        metadata.title = non_empty(tag.get_string(ItemKey::TrackTitle));
        metadata.artists = collect_artists(tag);
        metadata.album_artist = non_empty(tag.get_string(ItemKey::AlbumArtist));
        metadata.album = non_empty(tag.get_string(ItemKey::AlbumTitle));
        metadata.track_no = tag
            .get_string(ItemKey::TrackNumber)
            .and_then(parse_number_prefix)
            .or_else(|| tag.track());
        metadata.disc_no = tag
            .get_string(ItemKey::DiscNumber)
            .and_then(parse_number_prefix)
            .or_else(|| tag.disk());
        metadata.release_year = release_year(tag);
        metadata.artwork = first_artwork(tag);
    }

    Ok(metadata)
}

/// Detecta o contêiner pelo conteúdo (magic bytes), independente da extensão.
pub fn sniff_container(path: &Path) -> Option<String> {
    let header = read_header(path).ok()?;
    container_from_header(&header)
}

fn container_from_header(header: &[u8]) -> Option<String> {
    if let Some(file_type) = FileType::from_buffer(header) {
        return Some(
            match file_type {
                FileType::Opus | FileType::Vorbis | FileType::Speex => "ogg",
                FileType::Flac => "flac",
                FileType::Mpeg => "mp3",
                FileType::Mp4 => "mp4",
                FileType::Aac => "aac",
                FileType::Wav => "wav",
                FileType::Aiff => "aiff",
                FileType::Ape => "ape",
                FileType::Mpc => "mpc",
                FileType::WavPack => "wavpack",
                _ => return infer_container(header),
            }
            .to_string(),
        );
    }

    infer_container(header)
}

fn infer_container(header: &[u8]) -> Option<String> {
    let kind = infer::get(header)?;
    let extension = kind.extension();
    if extension.is_empty() {
        None
    } else {
        Some(extension.to_string())
    }
}

fn read_header(path: &Path) -> Result<Vec<u8>, MetadataError> {
    let mut file = File::open(path)?;
    let mut buffer = [0_u8; HEADER_BYTES];
    let mut filled = 0;

    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(MetadataError::Io(error)),
        }
    }

    Ok(buffer[..filled].to_vec())
}

fn parse_error(error: FileParseError) -> MetadataError {
    if error.is_unknown_format() {
        return MetadataError::Corrupt("formato de áudio desconhecido".to_string());
    }

    if let Some(source) = std::error::Error::source(&error)
        && let Some(io_error) = source.downcast_ref::<io::Error>()
    {
        return MetadataError::Io(io::Error::new(io_error.kind(), io_error.to_string()));
    }

    MetadataError::Corrupt(error.to_string())
}

fn file_type_name(file_type: FileType) -> String {
    let name = match file_type {
        FileType::Opus => "opus",
        FileType::Vorbis => "vorbis",
        FileType::Speex => "speex",
        FileType::Flac => "flac",
        FileType::Mpeg => "mp3",
        FileType::Mp4 => "mp4",
        FileType::Aac => "aac",
        FileType::Wav => "wav",
        FileType::Aiff => "aiff",
        FileType::Ape => "ape",
        FileType::Mpc => "mpc",
        FileType::WavPack => "wavpack",
        other => return format!("{other:?}").to_ascii_lowercase(),
    };

    name.to_string()
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn collect_artists(tag: &Tag) -> Vec<String> {
    let mut artists: Vec<String> = tag
        .get_strings(ItemKey::TrackArtist)
        .filter_map(|value| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        })
        .collect();

    if artists.is_empty() {
        artists = tag
            .get_strings(ItemKey::TrackArtists)
            .filter_map(|value| {
                let value = value.trim();
                (!value.is_empty()).then(|| value.to_string())
            })
            .collect();
    }

    artists
}

fn parse_number_prefix(value: &str) -> Option<u32> {
    let digits: String = value
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

fn parse_year_prefix(value: &str) -> Option<i32> {
    let digits: String = value
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .take(4)
        .collect();
    digits.parse().ok()
}

fn release_year(tag: &Tag) -> Option<i32> {
    if let Some(timestamp) = tag.date() {
        return Some(i32::from(timestamp.year));
    }

    tag.get_string(ItemKey::Year)
        .and_then(parse_year_prefix)
        .or_else(|| {
            tag.get_string(ItemKey::RecordingDate)
                .and_then(parse_year_prefix)
        })
}

fn first_artwork(tag: &Tag) -> Option<ExtractedArtwork> {
    let pictures = tag.pictures();
    let picture = pictures
        .iter()
        .find(|picture| picture.pic_type() == PictureType::CoverFront)
        .or_else(|| pictures.first())?;

    let bytes = picture.data().to_vec();
    if bytes.is_empty() {
        return None;
    }

    Some(ExtractedArtwork {
        bytes,
        mime_type: picture
            .mime_type()
            .map(|mime| mime.as_str().to_string())
            .filter(|mime| !mime.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_from_header_detects_ogg() {
        let mut header = vec![0_u8; HEADER_BYTES];
        header[..4].copy_from_slice(b"OggS");
        header[28..36].copy_from_slice(b"OpusHead");
        assert_eq!(container_from_header(&header).as_deref(), Some("ogg"));
    }

    #[test]
    fn container_from_header_detects_flac_and_mp3() {
        let mut flac = vec![0_u8; HEADER_BYTES];
        flac[..4].copy_from_slice(b"fLaC");
        assert_eq!(container_from_header(&flac).as_deref(), Some("flac"));

        let mp3 = b"ID3\x04\x00\x00\x00\x00\x00\x00rest";
        assert_eq!(container_from_header(mp3).as_deref(), Some("mp3"));
    }

    #[test]
    fn container_from_header_rejects_garbage() {
        assert_eq!(container_from_header(b"ZUNO-BROKEN"), None);
    }

    #[test]
    fn number_prefix_accepts_slash_notation() {
        assert_eq!(parse_number_prefix("7/12"), Some(7));
        assert_eq!(parse_number_prefix(" 3 "), Some(3));
        assert_eq!(parse_number_prefix("A1"), None);
        assert_eq!(parse_year_prefix("2024-05-01"), Some(2024));
    }
}

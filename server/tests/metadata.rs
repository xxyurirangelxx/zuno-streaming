//! Testes de extração de metadados usando as fixtures de `fixtures/audio/`.
//!
//! As fixtures são geradas por `scripts/generate-fixtures.py` e versionadas.

use std::path::{Path, PathBuf};

use zuno_server::library::metadata::{self, MetadataError};

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join("audio")
        .join(name)
}

#[test]
fn tone_a_opus_exposes_tags_properties_and_artwork() {
    let metadata =
        metadata::extract(&fixture_path("tone-a.opus")).expect("tone-a.opus deve ser válido");

    assert_eq!(metadata.codec, "opus");
    assert_eq!(metadata.container, "ogg");

    assert_eq!(metadata.title.as_deref(), Some("Tone A"));
    assert_eq!(metadata.artists, vec!["Zuno Test".to_string()]);
    assert_eq!(metadata.album_artist.as_deref(), Some("Zuno Test"));
    assert_eq!(metadata.album.as_deref(), Some("Fixture Album"));
    assert_eq!(metadata.track_no, Some(1));
    assert_eq!(metadata.disc_no, Some(1));
    assert_eq!(metadata.release_year, Some(2024));

    let duration_ms = metadata.duration_ms.expect("duração presente");
    assert!(
        duration_ms > 0,
        "duração deve ser maior que zero: {duration_ms}"
    );

    let bitrate = metadata.bitrate_kbps.expect("bitrate presente");
    assert!(
        (32..=320).contains(&bitrate),
        "bitrate fora da faixa esperada para 128 kbps VBR: {bitrate}"
    );

    assert_eq!(metadata.sample_rate_hz, Some(48_000));
    assert_eq!(metadata.channels, Some(1));

    let artwork = metadata.artwork.expect("capa embutida presente");
    assert!(
        !artwork.bytes.is_empty(),
        "bytes da capa devem ser não vazios"
    );
    assert_eq!(artwork.mime_type.as_deref(), Some("image/png"));
    assert!(
        artwork
            .bytes
            .starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
        "capa deve ser um PNG válido"
    );
}

#[test]
fn no_tags_opus_has_no_tags_but_keeps_duration() {
    let metadata =
        metadata::extract(&fixture_path("no-tags.opus")).expect("no-tags.opus deve ser válido");

    assert_eq!(metadata.codec, "opus");
    assert_eq!(metadata.title, None);
    assert!(
        metadata.artists.is_empty(),
        "artistas: {:?}",
        metadata.artists
    );
    assert_eq!(metadata.album_artist, None);
    assert_eq!(metadata.album, None);
    assert_eq!(metadata.track_no, None);
    assert_eq!(metadata.release_year, None);
    assert_eq!(metadata.artwork, None);

    let duration_ms = metadata.duration_ms.expect("duração presente");
    assert!(
        duration_ms > 0,
        "duração deve ser maior que zero: {duration_ms}"
    );
}

#[test]
fn duplicate_is_byte_identical_to_tone_a() {
    let original = std::fs::read(fixture_path("tone-a.opus")).expect("tone-a.opus");
    let duplicate = std::fs::read(fixture_path("duplicate-a.opus")).expect("duplicate-a.opus");

    assert_eq!(original, duplicate);
    assert_eq!(
        metadata::extract(&fixture_path("duplicate-a.opus"))
            .expect("duplicate-a.opus deve ser válido"),
        metadata::extract(&fixture_path("tone-a.opus")).expect("tone-a.opus deve ser válido")
    );
}

#[test]
fn vorbis_is_reported_as_unsupported() {
    let error = metadata::extract(&fixture_path("vorbis.ogg")).expect_err("Vorbis não é suportado");

    match error {
        MetadataError::Unsupported(kind) => {
            assert!(
                kind.contains("vorbis"),
                "mensagem deveria citar vorbis: {kind}"
            );
        }
        other => panic!("esperava Unsupported, veio {other:?}"),
    }
}

#[test]
fn broken_bytes_are_reported_as_corrupt() {
    let error = metadata::extract(&fixture_path("broken.opus"))
        .expect_err("bytes aleatórios não são áudio");

    match error {
        MetadataError::Corrupt(message) => {
            assert!(!message.is_empty(), "mensagem de corrupção vazia");
        }
        other => panic!("esperava Corrupt, veio {other:?}"),
    }
}

#[test]
fn missing_file_is_reported_as_io() {
    let error =
        metadata::extract(&fixture_path("does-not-exist.opus")).expect_err("arquivo inexistente");

    match error {
        MetadataError::Io(io_error) => {
            assert_eq!(io_error.kind(), std::io::ErrorKind::NotFound);
        }
        other => panic!("esperava Io, veio {other:?}"),
    }
}

#[test]
fn sniff_container_identifies_ogg_and_ignores_extension() {
    assert_eq!(
        metadata::sniff_container(&fixture_path("tone-a.opus")).as_deref(),
        Some("ogg")
    );
    assert_eq!(
        metadata::sniff_container(&fixture_path("vorbis.ogg")).as_deref(),
        Some("ogg")
    );
    assert_eq!(
        metadata::sniff_container(&fixture_path("broken.opus")),
        None
    );
    assert_eq!(
        metadata::sniff_container(&fixture_path("does-not-exist.opus")),
        None
    );
}

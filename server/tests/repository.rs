//! Testes de integração da camada de repositório (M1a).

use tempfile::TempDir;
use zuno_server::domain::{AudioFileDraft, ScanIssue, ScanIssueCode, TrackDraft, normalize_key};
use zuno_server::repository::{self, ArtworkDraft, ScanCounts};
use zuno_server::{Config, Database};

struct TestContext {
    _dir: TempDir,
    db: Database,
}

async fn test_context() -> TestContext {
    let dir = tempfile::tempdir().expect("diretório temporário");
    let db_path = dir.path().join("zuno.db");
    let database_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
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

    let db = Database::connect(&config)
        .await
        .expect("banco de teste deve conectar");

    TestContext { _dir: dir, db }
}

fn artwork() -> ArtworkDraft {
    ArtworkDraft {
        relative_path: "abcdef.jpg".to_string(),
        source: "embedded".to_string(),
        mime_type: Some("image/jpeg".to_string()),
        checksum: "abcdef".to_string(),
    }
}

fn track_draft(id: &str, title: &str, album_id: Option<String>, artist_id: &str) -> TrackDraft {
    TrackDraft {
        id: id.to_string(),
        album_id,
        title: title.to_string(),
        normalized_title: normalize_key(title),
        disc_no: Some(1),
        track_no: Some(3),
        duration_ms: Some(185_000),
        artists: vec![(artist_id.to_string(), "performer".to_string(), 0)],
    }
}

fn audio_file_draft(id: &str, track_id: &str, relative_path: &str, sha256: &str) -> AudioFileDraft {
    AudioFileDraft {
        id: id.to_string(),
        track_id: track_id.to_string(),
        relative_path: relative_path.to_string(),
        sha256: sha256.to_string(),
        codec: "opus".to_string(),
        container: "ogg".to_string(),
        bitrate: Some(128),
        sample_rate: Some(48_000),
        channels: Some(2),
        size_bytes: 4_096,
        source_mtime: 1_700_000_000,
        available: true,
    }
}

async fn count(pool: &sqlx::SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .expect("contagem deve funcionar")
}

#[tokio::test]
async fn upsert_artist_is_idempotent_and_keeps_first_name() {
    let context = test_context().await;
    let pool = context.db.pool();

    let first = repository::upsert_artist(pool, "Daft Punk")
        .await
        .expect("artista");
    let same = repository::upsert_artist(pool, "  daft   punk ")
        .await
        .expect("artista equivalente");
    assert_eq!(
        first, same,
        "mesmo nome normalizado deve reutilizar a linha"
    );

    let other = repository::upsert_artist(pool, "Daft Punk (Live)")
        .await
        .expect("artista distinto");
    assert_ne!(first, other);

    assert_eq!(count(pool, "artists").await, 2);

    let stored: String = sqlx::query_scalar("SELECT name FROM artists WHERE id = ?1")
        .bind(&first)
        .fetch_one(pool)
        .await
        .expect("nome gravado");
    assert_eq!(stored, "Daft Punk", "o nome original não é sobrescrito");
}

#[tokio::test]
async fn upsert_album_is_idempotent_and_fills_missing_fields() {
    let context = test_context().await;
    let pool = context.db.pool();

    let artist_id = repository::upsert_artist(pool, "Caetano Veloso")
        .await
        .expect("artista");

    let album_id = repository::upsert_album(pool, "Transa", Some(&artist_id), None, None)
        .await
        .expect("álbum");
    let again = repository::upsert_album(pool, "  transa ", Some(&artist_id), Some(1972), None)
        .await
        .expect("álbum repetido");
    assert_eq!(album_id, again);
    assert_eq!(count(pool, "albums").await, 1);

    let (year, artwork_id): (Option<i64>, Option<String>) =
        sqlx::query_as("SELECT release_year, artwork_id FROM albums WHERE id = ?1")
            .bind(&album_id)
            .fetch_one(pool)
            .await
            .expect("álbum gravado");
    assert_eq!(year, Some(1972), "ano preenche apenas quando estava vazio");
    assert_eq!(artwork_id, None);

    let artwork_id = repository::upsert_artwork(pool, &artwork())
        .await
        .expect("capa");
    let with_artwork = repository::upsert_album(
        pool,
        "Transa",
        Some(&artist_id),
        Some(1999),
        Some(&artwork_id),
    )
    .await
    .expect("álbum com capa");
    assert_eq!(with_artwork, album_id);

    let (year, stored_artwork): (Option<i64>, Option<String>) =
        sqlx::query_as("SELECT release_year, artwork_id FROM albums WHERE id = ?1")
            .bind(&album_id)
            .fetch_one(pool)
            .await
            .expect("álbum gravado");
    assert_eq!(year, Some(1972), "ano existente não é sobrescrito");
    assert_eq!(stored_artwork, Some(artwork_id));

    // Álbum sem artista principal também é idempotente.
    let orphan = repository::upsert_album(pool, "Coletânea", None, None, None)
        .await
        .expect("álbum sem artista");
    let orphan_again = repository::upsert_album(pool, "Coletânea", None, None, None)
        .await
        .expect("álbum sem artista repetido");
    assert_eq!(orphan, orphan_again);
    assert_eq!(count(pool, "albums").await, 2);
}

#[tokio::test]
async fn upsert_artwork_is_idempotent_by_checksum() {
    let context = test_context().await;
    let pool = context.db.pool();

    let draft = artwork();
    let first = repository::upsert_artwork(pool, &draft)
        .await
        .expect("capa");
    let second = repository::upsert_artwork(pool, &draft)
        .await
        .expect("capa repetida");
    assert_eq!(first, second);
    assert_eq!(count(pool, "artworks").await, 1);

    // Mesmo conteúdo cacheado em outro caminho continua sendo a mesma capa.
    let same_checksum = ArtworkDraft {
        relative_path: "outro-nome.jpg".to_string(),
        ..draft
    };
    let third = repository::upsert_artwork(pool, &same_checksum)
        .await
        .expect("capa por checksum");
    assert_eq!(first, third);
    assert_eq!(count(pool, "artworks").await, 1);
}

#[tokio::test]
async fn persist_track_with_file_creates_all_entities_and_is_idempotent() {
    let context = test_context().await;
    let pool = context.db.pool();

    let artist_id = repository::upsert_artist(pool, "Canção Nova")
        .await
        .expect("artista");
    let album_id = repository::upsert_album(pool, "Álbum Teste", Some(&artist_id), None, None)
        .await
        .expect("álbum");

    let track = track_draft("track-1", "Canção", Some(album_id.clone()), &artist_id);
    let file = audio_file_draft("file-1", "track-1", "Cancao/Album/01.opus", "sha-1");

    repository::persist_track_with_file(pool, &track, &file, Some(&artwork()))
        .await
        .expect("persistência");

    assert_eq!(count(pool, "tracks").await, 1);
    assert_eq!(count(pool, "track_artists").await, 1);
    assert_eq!(count(pool, "audio_files").await, 1);
    assert_eq!(count(pool, "artworks").await, 1);
    assert_eq!(count(pool, "tracks_fts").await, 1);

    let stored_artwork: Option<String> =
        sqlx::query_scalar("SELECT artwork_id FROM albums WHERE id = ?1")
            .bind(&album_id)
            .fetch_one(pool)
            .await
            .expect("álbum");
    assert!(stored_artwork.is_some(), "álbum deve receber a capa");

    // Reexecução: nada é duplicado.
    repository::persist_track_with_file(pool, &track, &file, Some(&artwork()))
        .await
        .expect("persistência repetida");

    assert_eq!(count(pool, "tracks").await, 1);
    assert_eq!(count(pool, "track_artists").await, 1);
    assert_eq!(count(pool, "audio_files").await, 1);
    assert_eq!(count(pool, "artworks").await, 1);
    assert_eq!(count(pool, "tracks_fts").await, 1);

    let found = repository::search_tracks(pool, "cancao", 10)
        .await
        .expect("busca");
    assert_eq!(found, vec!["track-1".to_string()]);
}

#[tokio::test]
async fn upsert_track_replaces_artists_and_resyncs_fts() {
    let context = test_context().await;
    let pool = context.db.pool();

    let first_artist = repository::upsert_artist(pool, "Tom Jobim")
        .await
        .expect("artista");
    let second_artist = repository::upsert_artist(pool, "Elis Regina")
        .await
        .expect("artista");

    let mut track = track_draft("track-x", "Águas de Março", None, &first_artist);
    repository::upsert_track(pool, &track).await.expect("faixa");
    assert_eq!(count(pool, "track_artists").await, 1);

    track.title = "Águas de Março (Dueto)".to_string();
    track.normalized_title = normalize_key(&track.title);
    track.artists = vec![
        (first_artist.clone(), "performer".to_string(), 0),
        (second_artist.clone(), "performer".to_string(), 1),
    ];
    repository::upsert_track(pool, &track)
        .await
        .expect("faixa atualizada");

    assert_eq!(count(pool, "tracks").await, 1);
    assert_eq!(count(pool, "track_artists").await, 2);
    assert_eq!(
        count(pool, "tracks_fts").await,
        1,
        "FTS não deve acumular linhas órfãs"
    );

    let by_artist = repository::search_tracks(pool, "elis", 10)
        .await
        .expect("busca por artista");
    assert_eq!(by_artist, vec!["track-x".to_string()]);
}

#[tokio::test]
async fn search_tracks_matches_title_and_artist_without_accents() {
    let context = test_context().await;
    let pool = context.db.pool();

    let artist_id = repository::upsert_artist(pool, "João Gilberto")
        .await
        .expect("artista");
    let album_id = repository::upsert_album(pool, "Chega de Saudade", Some(&artist_id), None, None)
        .await
        .expect("álbum");

    let track = track_draft("track-acento", "Canção de Amor", Some(album_id), &artist_id);
    let file = audio_file_draft("file-acento", "track-acento", "joao/01.opus", "sha-acento");
    repository::persist_track_with_file(pool, &track, &file, None)
        .await
        .expect("persistência");

    let by_title = repository::search_tracks(pool, "cancao", 10)
        .await
        .expect("busca sem acento no título");
    assert_eq!(by_title, vec!["track-acento".to_string()]);

    let by_accented_title = repository::search_tracks(pool, "canção", 10)
        .await
        .expect("busca com acento no título");
    assert_eq!(by_accented_title, vec!["track-acento".to_string()]);

    let by_artist = repository::search_tracks(pool, "joao", 10)
        .await
        .expect("busca sem acento no artista");
    assert_eq!(by_artist, vec!["track-acento".to_string()]);

    let by_album = repository::search_tracks(pool, "saudade", 10)
        .await
        .expect("busca pelo álbum");
    assert_eq!(by_album, vec!["track-acento".to_string()]);

    let prefix = repository::search_tracks(pool, "canc", 10)
        .await
        .expect("busca por prefixo");
    assert_eq!(prefix, vec!["track-acento".to_string()]);

    let missing = repository::search_tracks(pool, "beatles", 10)
        .await
        .expect("busca sem resultado");
    assert!(missing.is_empty());
}

#[tokio::test]
async fn persist_track_with_file_rolls_back_on_error() {
    let context = test_context().await;
    let pool = context.db.pool();

    let artist_id = repository::upsert_artist(pool, "Artista Rollback")
        .await
        .expect("artista");

    // Álbum inexistente viola a FK: a transação inteira deve ser desfeita.
    let track = track_draft(
        "track-rollback",
        "Faixa Órfã",
        Some("album-inexistente".to_string()),
        &artist_id,
    );
    let file = audio_file_draft("file-rollback", "track-rollback", "rb/01.opus", "sha-rb");

    let result = repository::persist_track_with_file(pool, &track, &file, Some(&artwork())).await;
    assert!(result.is_err(), "FK inválida deve falhar");

    assert_eq!(count(pool, "tracks").await, 0);
    assert_eq!(count(pool, "track_artists").await, 0);
    assert_eq!(count(pool, "audio_files").await, 0);
    assert_eq!(count(pool, "tracks_fts").await, 0);
    assert_eq!(
        count(pool, "artworks").await,
        0,
        "a capa gravada antes também é desfeita"
    );
}

#[tokio::test]
async fn search_tracks_ignores_empty_query_and_clamps_limit() {
    let context = test_context().await;
    let pool = context.db.pool();

    let artist_id = repository::upsert_artist(pool, "Zé Ramalho")
        .await
        .expect("artista");

    for index in 0..3 {
        let id = format!("track-{index}");
        let track = track_draft(&id, "Admirável Gado Novo", None, &artist_id);
        repository::upsert_track(pool, &track).await.expect("faixa");
    }

    assert!(
        repository::search_tracks(pool, "", 10)
            .await
            .expect("query vazia")
            .is_empty()
    );
    assert!(
        repository::search_tracks(pool, "   ", 10)
            .await
            .expect("query em branco")
            .is_empty()
    );
    assert!(
        repository::search_tracks(pool, "-*\"", 10)
            .await
            .expect("query só com pontuação")
            .is_empty()
    );

    let all = repository::search_tracks(pool, "admiravel", 100)
        .await
        .expect("busca");
    assert_eq!(all.len(), 3);

    let limited = repository::search_tracks(pool, "admirável", 0)
        .await
        .expect("limite zero é clampado para 1");
    assert_eq!(limited.len(), 1);
}

#[tokio::test]
async fn find_audio_file_by_sha_returns_indexed_file() {
    let context = test_context().await;
    let pool = context.db.pool();

    let artist_id = repository::upsert_artist(pool, "Nina Simone")
        .await
        .expect("artista");
    let track = track_draft("track-sha", "Feeling Good", None, &artist_id);
    let file = audio_file_draft("file-sha", "track-sha", "nina/01.opus", "sha-unico");
    repository::persist_track_with_file(pool, &track, &file, None)
        .await
        .expect("persistência");

    let found = repository::find_audio_file_by_sha(pool, "sha-unico")
        .await
        .expect("busca por sha");
    assert_eq!(found, Some("file-sha".to_string()));

    let missing = repository::find_audio_file_by_sha(pool, "sha-inexistente")
        .await
        .expect("busca por sha inexistente");
    assert_eq!(missing, None);
}

#[tokio::test]
async fn load_audio_file_states_includes_track_id() {
    let context = test_context().await;
    let pool = context.db.pool();

    let artist_id = repository::upsert_artist(pool, "Gal Costa")
        .await
        .expect("artista");
    let track = track_draft("track-state", "Baby", None, &artist_id);
    let file = audio_file_draft("file-state", "track-state", "gal/01.opus", "sha-state");
    repository::persist_track_with_file(pool, &track, &file, None)
        .await
        .expect("persistência");

    let states = repository::load_audio_file_states(pool)
        .await
        .expect("estados");
    let state = states.get("gal/01.opus").expect("estado do arquivo");

    assert_eq!(state.id, "file-state");
    assert_eq!(state.track_id, "track-state");
    assert_eq!(state.sha256, "sha-state");
    assert_eq!(state.size_bytes, 4_096);
    assert_eq!(state.source_mtime, 1_700_000_000);
    assert!(state.available);
}

#[tokio::test]
async fn mark_missing_audio_files_marks_unavailable_without_deleting() {
    let context = test_context().await;
    let pool = context.db.pool();

    let artist_id = repository::upsert_artist(pool, "Milton Nascimento")
        .await
        .expect("artista");

    for (index, path) in ["milton/01.opus", "milton/02.opus", "milton/03.opus"]
        .iter()
        .enumerate()
    {
        let track_id = format!("track-{index}");
        let track = track_draft(&track_id, "Travessia", None, &artist_id);
        let file = audio_file_draft(
            &format!("file-{index}"),
            &track_id,
            path,
            &format!("sha-{index}"),
        );
        repository::persist_track_with_file(pool, &track, &file, None)
            .await
            .expect("persistência");
    }

    let marked = repository::mark_missing_audio_files(
        pool,
        &["milton/01.opus".to_string(), "milton/03.opus".to_string()],
    )
    .await
    .expect("marcação");

    assert_eq!(marked, 1, "apenas o arquivo ausente muda de estado");
    assert_eq!(count(pool, "audio_files").await, 3, "nada é apagado");

    let unavailable: Vec<String> =
        sqlx::query_scalar("SELECT relative_path FROM audio_files WHERE available = 0")
            .fetch_all(pool)
            .await
            .expect("arquivos indisponíveis");
    assert_eq!(unavailable, vec!["milton/02.opus".to_string()]);

    // Reexecutar não rebaixa nada de novo.
    let again = repository::mark_missing_audio_files(
        pool,
        &["milton/01.opus".to_string(), "milton/03.opus".to_string()],
    )
    .await
    .expect("marcação repetida");
    assert_eq!(again, 0);

    let states = repository::load_audio_file_states(pool)
        .await
        .expect("estados");
    assert!(!states["milton/02.opus"].available);
    assert!(states["milton/01.opus"].available);

    // Arquivo que volta aparece como visto novamente.
    repository::upsert_audio_file(
        pool,
        &audio_file_draft("file-1", "track-1", "milton/02.opus", "sha-1"),
    )
    .await
    .expect("reativação");
    let states = repository::load_audio_file_states(pool)
        .await
        .expect("estados");
    assert!(states["milton/02.opus"].available);
}

#[tokio::test]
async fn mark_missing_audio_files_handles_large_path_sets() {
    let context = test_context().await;
    let pool = context.db.pool();

    let artist_id = repository::upsert_artist(pool, "Lote Grande")
        .await
        .expect("artista");
    let track = track_draft("track-lote", "Faixa", None, &artist_id);
    let file = audio_file_draft("file-lote", "track-lote", "lote/0000.opus", "sha-lote");
    repository::persist_track_with_file(pool, &track, &file, None)
        .await
        .expect("persistência");

    let seen: Vec<String> = (0..1_200)
        .map(|index| format!("lote/{index:04}.opus"))
        .collect();
    let marked = repository::mark_missing_audio_files(pool, &seen)
        .await
        .expect("marcação em chunks");
    assert_eq!(marked, 0, "o arquivo existente está na lista de vistos");

    let marked = repository::mark_missing_audio_files(pool, &[])
        .await
        .expect("nenhum arquivo visto");
    assert_eq!(marked, 1);
}

#[tokio::test]
async fn scan_run_lifecycle_tracks_completed_and_failed() {
    let context = test_context().await;
    let pool = context.db.pool();

    repository::start_scan_run(pool, "run-ok")
        .await
        .expect("início do scan");
    let state: String = sqlx::query_scalar("SELECT state FROM scan_runs WHERE id = ?1")
        .bind("run-ok")
        .fetch_one(pool)
        .await
        .expect("linha do scan");
    assert_eq!(state, "running");

    repository::record_scan_issue(
        pool,
        "run-ok",
        &ScanIssue {
            relative_path: "x/track.mp3".to_string(),
            code: ScanIssueCode::UnsupportedFormat,
            detail: Some("converta para Opus".to_string()),
        },
    )
    .await
    .expect("problema registrado");

    repository::finish_scan_run(
        pool,
        "run-ok",
        ScanCounts {
            files_seen: 10,
            files_indexed: 7,
            files_unchanged: 1,
            files_skipped: 1,
            files_failed: 0,
            files_missing: 1,
        },
        None,
    )
    .await
    .expect("fim do scan");

    let (state, finished, seen, indexed, missing, error): (
        String,
        Option<String>,
        i64,
        i64,
        i64,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT state, finished_at, files_seen, files_indexed, files_missing, error
         FROM scan_runs WHERE id = ?1",
    )
    .bind("run-ok")
    .fetch_one(pool)
    .await
    .expect("scan concluído");

    assert_eq!(state, "completed");
    assert!(finished.is_some(), "finished_at deve ser preenchido");
    assert_eq!((seen, indexed, missing), (10, 7, 1));
    assert_eq!(error, None);

    let issues: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scan_issues WHERE scan_run_id = ?1")
        .bind("run-ok")
        .fetch_one(pool)
        .await
        .expect("problemas");
    assert_eq!(issues, 1);

    repository::start_scan_run(pool, "run-fail")
        .await
        .expect("início do scan com falha");
    repository::finish_scan_run(
        pool,
        "run-fail",
        ScanCounts::default(),
        Some("disco removido"),
    )
    .await
    .expect("fim do scan com falha");

    let (state, finished, error): (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT state, finished_at, error FROM scan_runs WHERE id = ?1")
            .bind("run-fail")
            .fetch_one(pool)
            .await
            .expect("scan com falha");

    assert_eq!(state, "failed");
    assert!(finished.is_some());
    assert_eq!(error.as_deref(), Some("disco removido"));

    // Finalizar uma varredura inexistente é um erro explícito.
    assert!(
        repository::finish_scan_run(pool, "run-inexistente", ScanCounts::default(), None)
            .await
            .is_err()
    );
}

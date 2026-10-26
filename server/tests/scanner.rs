use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use zuno_server::domain::ScanIssueCode;
use zuno_server::{Config, Database, LibraryScanner, ScanOptions};

const FIXTURES: [&str; 6] = [
    "tone-a.opus",
    "tone-b.opus",
    "no-tags.opus",
    "duplicate-a.opus",
    "vorbis.ogg",
    "broken.opus",
];

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/audio")
}

struct TestEnv {
    /// Mantém o diretório temporário vivo; lido diretamente no teste de symlink (unix).
    #[allow(dead_code)]
    dir: TempDir,
    database: Database,
    config: Config,
}

impl TestEnv {
    async fn new(max_audio_file_bytes: u64) -> Self {
        let dir = tempfile::tempdir().expect("diretório temporário");
        let music_dir = dir.path().join("music");
        fs::create_dir_all(&music_dir).expect("criar diretório de música");

        for name in FIXTURES {
            let source = fixtures_dir().join(name);
            assert!(
                source.exists(),
                "fixture ausente: {} (rode scripts/generate-fixtures.py)",
                source.display()
            );
            fs::copy(&source, music_dir.join(name)).expect("copiar fixture");
        }

        let database_url = format!(
            "sqlite://{}",
            dir.path()
                .join("zuno.db")
                .to_string_lossy()
                .replace('\\', "/")
        );
        let config = Config {
            bind_addr: "127.0.0.1:0".parse().expect("bind"),
            database_url,
            music_dir,
            artwork_dir: dir.path().join("artwork"),
            staging_dir: dir.path().join("staging"),
            max_audio_file_bytes,
            scan_on_startup: false,
            cookie_secure: false,
            db_max_connections: 2,
        };
        let database = Database::connect(&config)
            .await
            .expect("banco de teste deve conectar");

        Self {
            dir,
            database,
            config,
        }
    }

    fn scanner(&self) -> LibraryScanner {
        LibraryScanner::new(&self.config, ScanOptions::from_config(&self.config))
            .expect("scanner deve inicializar")
    }

    fn music(&self) -> PathBuf {
        self.config.music_dir.clone()
    }

    async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(self.database.pool())
            .await
            .expect("consulta de contagem")
    }
}

#[tokio::test]
async fn scan_indexes_opus_and_reports_unsupported_files() {
    let env = TestEnv::new(10 * 1024 * 1024).await;

    let report = env.scanner().scan(&env.database).await.expect("scan");

    assert_eq!(report.files_seen, 6, "todos os arquivos são visitados");
    assert_eq!(
        report.files_indexed, 4,
        "Opus válidos indexados (inclui duplicata)"
    );
    assert_eq!(report.files_failed, 1, "broken.opus falha");
    assert_eq!(report.files_skipped, 1, "vorbis.ogg é ignorado");

    let has = |code: ScanIssueCode, path: &str| {
        report
            .issues
            .iter()
            .any(|issue| issue.code == code && issue.relative_path == path)
    };

    assert!(has(ScanIssueCode::UnsupportedFormat, "vorbis.ogg"));
    assert!(has(ScanIssueCode::CorruptFile, "broken.opus"));
    assert_eq!(
        report
            .issues
            .iter()
            .filter(|issue| issue.code == ScanIssueCode::DuplicateExact)
            .count(),
        1,
        "o par byte-idêntico gera exatamente um aviso, independente da ordem do scan"
    );

    assert_eq!(env.count("SELECT COUNT(*) FROM tracks").await, 4);
    assert_eq!(env.count("SELECT COUNT(*) FROM audio_files").await, 4);
    assert_eq!(
        env.count("SELECT COUNT(*) FROM audio_files WHERE available = 1")
            .await,
        4
    );

    // Faixas com tags produzem artista e álbum; o arquivo sem tags usa fallback.
    assert!(env.count("SELECT COUNT(*) FROM artists").await >= 1);
    assert!(env.count("SELECT COUNT(*) FROM albums").await >= 1);
    assert_eq!(
        env.count("SELECT COUNT(*) FROM tracks WHERE title = 'no-tags'")
            .await,
        1,
        "fallback usa o nome do arquivo"
    );
}

#[tokio::test]
async fn rescan_is_idempotent_and_skips_rehashing() {
    let env = TestEnv::new(10 * 1024 * 1024).await;
    let scanner = env.scanner();

    let first = scanner.scan(&env.database).await.expect("primeiro scan");
    assert_eq!(first.files_indexed, 4);

    let second = scanner.scan(&env.database).await.expect("segundo scan");
    assert_eq!(second.files_indexed, 0, "nada é reprocessado");
    assert_eq!(second.files_unchanged, 4);

    assert_eq!(env.count("SELECT COUNT(*) FROM tracks").await, 4);
    assert_eq!(env.count("SELECT COUNT(*) FROM audio_files").await, 4);
}

#[tokio::test]
async fn missing_file_is_marked_unavailable_and_never_deleted() {
    let env = TestEnv::new(10 * 1024 * 1024).await;
    let scanner = env.scanner();
    scanner.scan(&env.database).await.expect("primeiro scan");

    fs::remove_file(env.music().join("tone-b.opus"))
        .expect("remover fixture do diretório de teste");

    let report = scanner.scan(&env.database).await.expect("segundo scan");
    assert_eq!(report.files_missing, 1);

    let available: i64 =
        sqlx::query_scalar("SELECT available FROM audio_files WHERE relative_path = 'tone-b.opus'")
            .fetch_one(env.database.pool())
            .await
            .expect("linha deve continuar existindo");
    assert_eq!(available, 0, "arquivo ausente vira indisponível");

    assert_eq!(
        env.count("SELECT COUNT(*) FROM tracks").await,
        4,
        "nenhuma faixa é apagada"
    );
}

#[tokio::test]
async fn oversized_files_are_reported_without_indexing() {
    let env = TestEnv::new(1024).await;

    let report = env.scanner().scan(&env.database).await.expect("scan");

    assert_eq!(report.files_indexed, 0);
    assert_eq!(
        report
            .issues
            .iter()
            .filter(|issue| issue.code == ScanIssueCode::TooLarge)
            .count(),
        6,
        "o limite de tamanho é aplicado antes de ler os metadados"
    );
    assert_eq!(env.count("SELECT COUNT(*) FROM tracks").await, 0);
}

#[tokio::test]
async fn artwork_is_cached_outside_the_library() {
    let env = TestEnv::new(10 * 1024 * 1024).await;
    env.scanner().scan(&env.database).await.expect("scan");

    assert_eq!(
        env.count("SELECT COUNT(*) FROM artworks").await,
        1,
        "tone-a e duplicate-a compartilham o mesmo checksum"
    );

    let artwork_dir = env.config.artwork_dir.clone();
    let files: Vec<_> = fs::read_dir(&artwork_dir)
        .expect("cache de capas deve existir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .collect();
    assert_eq!(files.len(), 1);
    assert!(
        files[0].metadata().expect("metadados").len() > 0,
        "capa não pode ser vazia"
    );

    let music_entries: Vec<_> = fs::read_dir(env.music())
        .expect("biblioteca")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .collect();
    assert!(
        !music_entries
            .iter()
            .any(|entry| entry.path().extension().is_some_and(|ext| ext == "png")),
        "o scanner nunca escreve na biblioteca"
    );
}

#[tokio::test]
async fn fts_finds_indexed_tracks() {
    let env = TestEnv::new(10 * 1024 * 1024).await;
    env.scanner().scan(&env.database).await.expect("scan");

    let hits = zuno_server::repository::search_tracks(env.database.pool(), "tone", 10)
        .await
        .expect("busca FTS");
    assert!(
        hits.len() >= 2,
        "esperava as faixas 'Tone A'/'Tone B', veio {hits:?}"
    );

    let empty = zuno_server::repository::search_tracks(env.database.pool(), "   ", 10)
        .await
        .expect("busca vazia");
    assert!(empty.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn symlinks_are_skipped() {
    use std::os::unix::fs::symlink;

    let env = TestEnv::new(10 * 1024 * 1024).await;
    let outside = env.dir.path().join("outside.opus");
    fs::copy(fixtures_dir().join("tone-a.opus"), &outside).expect("copiar fixture");
    symlink(&outside, env.music().join("link.opus")).expect("criar symlink");

    let report = env.scanner().scan(&env.database).await.expect("scan");

    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == ScanIssueCode::SymlinkSkipped),
        "symlink deve ser reportado: {:?}",
        report.issues
    );
}

//! Configuração tipada do servidor Zuno, carregada de variáveis de ambiente.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, anyhow};

/// Endereço de bind padrão: sempre local, nunca exposto publicamente.
pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8080";
/// URL padrão do banco SQLite.
pub const DEFAULT_DATABASE_URL: &str = "sqlite://data/zuno.db";
/// Diretório padrão da biblioteca musical (reservado ao M1).
pub const DEFAULT_MUSIC_DIR: &str = "./data/music";
/// Diretório padrão do cache de capas extraídas.
pub const DEFAULT_ARTWORK_DIR: &str = "./data/artwork";
/// Diretório padrão de staging de uploads (fora da biblioteca).
pub const DEFAULT_STAGING_DIR: &str = "./data/staging";
/// Limite padrão de tamanho por arquivo de áudio (500 MiB).
pub const DEFAULT_MAX_AUDIO_FILE_BYTES: u64 = 500 * 1024 * 1024;
/// Limite inferior aceito para `ZUNO_MAX_AUDIO_FILE_BYTES`.
pub const MIN_AUDIO_FILE_BYTES: u64 = 1024;
/// Limite superior aceito para `ZUNO_MAX_AUDIO_FILE_BYTES` (10 GiB).
pub const MAX_AUDIO_FILE_BYTES: u64 = 10 * 1024 * 1024 * 1024;
/// Número padrão de conexões no pool SQLite.
pub const DEFAULT_DB_MAX_CONNECTIONS: u32 = 4;

/// Limite inferior aceito para `ZUNO_DB_MAX_CONNECTIONS`.
pub const MIN_DB_MAX_CONNECTIONS: u32 = 1;
/// Limite superior aceito para `ZUNO_DB_MAX_CONNECTIONS`.
pub const MAX_DB_MAX_CONNECTIONS: u32 = 32;

/// Configuração de execução do servidor.
#[derive(Clone)]
pub struct Config {
    /// Interface e porta em que o servidor escuta.
    pub bind_addr: SocketAddr,
    /// URL do banco SQLite.
    pub database_url: String,
    /// Diretório reservado para a biblioteca musical.
    pub music_dir: PathBuf,
    /// Diretório do cache de capas extraídas.
    pub artwork_dir: PathBuf,
    /// Diretório de staging de uploads (nunca dentro de `ZUNO_MUSIC_DIR`).
    pub staging_dir: PathBuf,
    /// Tamanho máximo aceito por arquivo de áudio no scan.
    pub max_audio_file_bytes: u64,
    /// Executa uma varredura da biblioteca na inicialização do servidor.
    pub scan_on_startup: bool,
    /// Número máximo de conexões no pool SQLite.
    pub db_max_connections: u32,
}

impl Config {
    /// Carrega a configuração a partir das variáveis de ambiente do processo.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    /// Carrega a configuração usando um leitor de variáveis injetável.
    ///
    /// A injeção existe para permitir testes determinísticos sem mutar o
    /// ambiente global do processo.
    pub fn from_env_with<F>(read: F) -> anyhow::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let bind_raw =
            non_empty(read("ZUNO_BIND_ADDR")).unwrap_or_else(|| DEFAULT_BIND_ADDR.into());
        let bind_addr = bind_raw.parse::<SocketAddr>().with_context(|| {
            format!(
                "ZUNO_BIND_ADDR inválido: '{bind_raw}'. Use o formato IP:PORTA, \
                 por exemplo 127.0.0.1:8080."
            )
        })?;

        let database_url = non_empty(read("ZUNO_DATABASE_URL"))
            .unwrap_or_else(|| DEFAULT_DATABASE_URL.to_string());
        let music_dir = PathBuf::from(
            non_empty(read("ZUNO_MUSIC_DIR")).unwrap_or_else(|| DEFAULT_MUSIC_DIR.to_string()),
        );

        let artwork_dir = PathBuf::from(
            non_empty(read("ZUNO_ARTWORK_DIR")).unwrap_or_else(|| DEFAULT_ARTWORK_DIR.to_string()),
        );

        let staging_dir = PathBuf::from(
            non_empty(read("ZUNO_STAGING_DIR")).unwrap_or_else(|| DEFAULT_STAGING_DIR.to_string()),
        );

        let max_audio_file_bytes = match non_empty(read("ZUNO_MAX_AUDIO_FILE_BYTES")) {
            Some(raw) => parse_max_audio_file_bytes(&raw)?,
            None => DEFAULT_MAX_AUDIO_FILE_BYTES,
        };

        let scan_on_startup = match non_empty(read("ZUNO_SCAN_ON_STARTUP")) {
            Some(raw) => parse_bool("ZUNO_SCAN_ON_STARTUP", &raw)?,
            None => false,
        };

        let db_max_connections = match non_empty(read("ZUNO_DB_MAX_CONNECTIONS")) {
            Some(raw) => parse_max_connections(&raw)?,
            None => DEFAULT_DB_MAX_CONNECTIONS,
        };

        Ok(Self {
            bind_addr,
            database_url,
            music_dir,
            artwork_dir,
            staging_dir,
            max_audio_file_bytes,
            scan_on_startup,
            db_max_connections,
        })
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("bind_addr", &self.bind_addr)
            .field("database_url", &"<redacted>")
            .field("music_dir", &self.music_dir)
            .field("artwork_dir", &self.artwork_dir)
            .field("staging_dir", &self.staging_dir)
            .field("max_audio_file_bytes", &self.max_audio_file_bytes)
            .field("scan_on_startup", &self.scan_on_startup)
            .field("db_max_connections", &self.db_max_connections)
            .finish()
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
}

fn parse_max_connections(raw: &str) -> anyhow::Result<u32> {
    let value = raw.parse::<u32>().with_context(|| {
        format!("ZUNO_DB_MAX_CONNECTIONS inválido: '{raw}'. Informe um número inteiro.")
    })?;

    if !(MIN_DB_MAX_CONNECTIONS..=MAX_DB_MAX_CONNECTIONS).contains(&value) {
        return Err(anyhow!(
            "ZUNO_DB_MAX_CONNECTIONS deve estar entre {MIN_DB_MAX_CONNECTIONS} e \
             {MAX_DB_MAX_CONNECTIONS}; valor recebido: {value}."
        ));
    }

    Ok(value)
}

fn parse_max_audio_file_bytes(raw: &str) -> anyhow::Result<u64> {
    let value = raw.parse::<u64>().with_context(|| {
        format!("ZUNO_MAX_AUDIO_FILE_BYTES inválido: '{raw}'. Informe um número de bytes.")
    })?;

    if !(MIN_AUDIO_FILE_BYTES..=MAX_AUDIO_FILE_BYTES).contains(&value) {
        return Err(anyhow!(
            "ZUNO_MAX_AUDIO_FILE_BYTES deve estar entre {MIN_AUDIO_FILE_BYTES} e {MAX_AUDIO_FILE_BYTES} bytes; valor recebido: {value}."
        ));
    }

    Ok(value)
}

fn parse_bool(key: &str, raw: &str) -> anyhow::Result<bool> {
    match raw.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(anyhow!("{key} inválido: '{raw}'. Use true/false (ou 1/0).")),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn config_from(pairs: &[(&str, &str)]) -> anyhow::Result<Config> {
        let env: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        Config::from_env_with(|key| env.get(key).cloned())
    }

    #[test]
    fn defaults_are_applied_when_env_is_absent() {
        let config = config_from(&[]).expect("config padrão deve ser válida");

        assert_eq!(config.bind_addr.to_string(), DEFAULT_BIND_ADDR);
        assert_eq!(config.database_url, DEFAULT_DATABASE_URL);
        assert_eq!(config.music_dir, PathBuf::from(DEFAULT_MUSIC_DIR));
        assert_eq!(config.db_max_connections, DEFAULT_DB_MAX_CONNECTIONS);
    }

    #[test]
    fn env_values_override_defaults() {
        let config = config_from(&[
            ("ZUNO_BIND_ADDR", "127.0.0.1:18080"),
            ("ZUNO_DATABASE_URL", "sqlite://tmp/test.db"),
            ("ZUNO_MUSIC_DIR", "./tmp/music"),
            ("ZUNO_DB_MAX_CONNECTIONS", "8"),
        ])
        .expect("config com override deve ser válida");

        assert_eq!(config.bind_addr.to_string(), "127.0.0.1:18080");
        assert_eq!(config.database_url, "sqlite://tmp/test.db");
        assert_eq!(config.music_dir, PathBuf::from("./tmp/music"));
        assert_eq!(config.db_max_connections, 8);
    }

    #[test]
    fn empty_values_fall_back_to_defaults() {
        let config = config_from(&[
            ("ZUNO_BIND_ADDR", "   "),
            ("ZUNO_DATABASE_URL", ""),
            ("ZUNO_MUSIC_DIR", ""),
            ("ZUNO_DB_MAX_CONNECTIONS", ""),
        ])
        .expect("valores vazios devem usar os padrões");

        assert_eq!(config.bind_addr.to_string(), DEFAULT_BIND_ADDR);
        assert_eq!(config.database_url, DEFAULT_DATABASE_URL);
        assert_eq!(config.db_max_connections, DEFAULT_DB_MAX_CONNECTIONS);
    }

    #[test]
    fn invalid_max_connections_is_rejected() {
        for value in ["0", "33", "-1", "abc"] {
            let result = config_from(&[("ZUNO_DB_MAX_CONNECTIONS", value)]);
            assert!(
                result.is_err(),
                "ZUNO_DB_MAX_CONNECTIONS={value} deveria ser rejeitado"
            );
        }
    }

    #[test]
    fn scan_and_artwork_settings_have_defaults() {
        let config = config_from(&[]).expect("config padrão deve ser válida");

        assert_eq!(config.artwork_dir, PathBuf::from(DEFAULT_ARTWORK_DIR));
        assert_eq!(config.staging_dir, PathBuf::from(DEFAULT_STAGING_DIR));
        assert_eq!(config.max_audio_file_bytes, DEFAULT_MAX_AUDIO_FILE_BYTES);
        assert!(!config.scan_on_startup);
    }

    #[test]
    fn scan_and_artwork_settings_can_be_overridden() {
        let config = config_from(&[
            ("ZUNO_ARTWORK_DIR", "./tmp/covers"),
            ("ZUNO_STAGING_DIR", "./tmp/staging"),
            ("ZUNO_MAX_AUDIO_FILE_BYTES", "1048576"),
            ("ZUNO_SCAN_ON_STARTUP", "true"),
        ])
        .expect("overrides válidos");

        assert_eq!(config.artwork_dir, PathBuf::from("./tmp/covers"));
        assert_eq!(config.staging_dir, PathBuf::from("./tmp/staging"));
        assert_eq!(config.max_audio_file_bytes, 1_048_576);
        assert!(config.scan_on_startup);
    }

    #[test]
    fn invalid_scan_settings_are_rejected() {
        for value in ["0", "1023", "99999999999999", "abc"] {
            assert!(
                config_from(&[("ZUNO_MAX_AUDIO_FILE_BYTES", value)]).is_err(),
                "ZUNO_MAX_AUDIO_FILE_BYTES={value} deveria ser rejeitado"
            );
        }

        assert!(config_from(&[("ZUNO_SCAN_ON_STARTUP", "talvez")]).is_err());
    }

    #[test]
    fn invalid_bind_addr_is_rejected() {
        let result = config_from(&[("ZUNO_BIND_ADDR", "nao-e-um-endereco")]);
        assert!(result.is_err(), "bind inválido deveria ser rejeitado");
    }

    #[test]
    fn debug_does_not_expose_database_url() {
        let config = config_from(&[("ZUNO_DATABASE_URL", "sqlite://segredo/oculto.db")])
            .expect("config deve ser válida");
        let rendered = format!("{config:?}");

        assert!(!rendered.contains("segredo"));
        assert!(rendered.contains("<redacted>"));
    }
}

use std::env;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub environment: String,
    pub database_path: PathBuf,
    pub log_level: String,
    pub server: ServerConfig,
    pub indexer: IndexerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub bearer_token_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    pub parallelism: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            environment: "development".to_string(),
            database_path: PathBuf::from(".data/index.dev.db"),
            log_level: "debug".to_string(),
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8765,
                bearer_token_env: Some("ODOO_KNOWLEDGE_BEARER_TOKEN".to_string()),
            },
            indexer: IndexerConfig { parallelism: 4 },
        }
    }
}

impl AppConfig {
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let env_path = env::var_os("ODOO_KNOWLEDGE_CONFIG").map(PathBuf::from);
        let chosen = config_path.map(PathBuf::from).or(env_path);
        let mut config = if let Some(path) = chosen {
            if !path.exists() {
                return Err(Error::ConfigNotFound(path));
            }
            let text = std::fs::read_to_string(path)?;
            toml::from_str::<AppConfig>(&text)?
        } else {
            AppConfig::default()
        };

        if let Some(value) = env::var_os("ODOO_KNOWLEDGE_ENV") {
            config.environment = value.to_string_lossy().to_string();
        }
        if let Some(value) = env::var_os("ODOO_KNOWLEDGE_DB") {
            config.database_path = PathBuf::from(value);
        }
        if let Some(value) = env::var_os("RUST_LOG") {
            config.log_level = value.to_string_lossy().to_string();
        }
        if config.indexer.parallelism == 0 {
            return Err(Error::InvalidConfig(
                "indexer.parallelism must be greater than zero".to_string(),
            ));
        }
        Ok(config)
    }
}

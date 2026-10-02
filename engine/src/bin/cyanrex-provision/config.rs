use super::{private_file, Result};
use cyanrex_engine::models::collaboration::{
    AuthorityId, LegacyUsername, WorkspaceId, WorkspaceRef,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

// Secret-bearing input intentionally has no Debug or Serialize. Public reports are explicit allowlists.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub format_version: u8,
    pub transport: Transport,
    pub port: u16,
    pub database: String,
    pub database_user: String,
    pub database_password: String,
    pub schema: String,
    pub authority_id: AuthorityId,
    pub workspace_id: WorkspaceId,
    pub username: LegacyUsername,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transport {
    Unix { directory: PathBuf },
    Loopback { address: String },
}
impl Config {
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = private_file::read(path, 16384)?;
        let config: Self = serde_json::from_slice(&bytes).map_err(|_| "invalid_config")?;
        let identifier = |s: &str| {
            !s.is_empty()
                && s.len() <= 63
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        };
        let schema = config.schema.as_bytes();
        if config.format_version != 1
            || config.port == 0
            || !identifier(&config.database)
            || !identifier(&config.database_user)
            || config.database_password.len() > 4096
            || config.database_password.contains('\0')
            || config.schema.len() > 63
            || !schema
                .first()
                .is_some_and(|b| b.is_ascii_lowercase() || *b == b'_')
            || !schema
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
            || config.schema.starts_with("pg_")
            || ["public", "information_schema"].contains(&config.schema.as_str())
        {
            return Err("invalid_config");
        }
        match &config.transport {
            Transport::Unix { directory }
                if directory.is_absolute()
                    && directory.as_os_str().len() <= 4096
                    && directory
                        .components()
                        .all(|c| matches!(c, Component::RootDir | Component::Normal(_))) => {}
            Transport::Loopback { address } if ["127.0.0.1", "::1"].contains(&address.as_str()) => {
            }
            _ => return Err("invalid_config"),
        }
        Ok(config)
    }
    pub fn scope(&self) -> WorkspaceRef {
        WorkspaceRef {
            authority_id: self.authority_id,
            workspace_id: self.workspace_id,
        }
    }
    pub fn public_target(&self) -> Value {
        json!({"transport":self.transport,"port":self.port,"database":self.database,
            "database_user":self.database_user,"schema":self.schema,
            "authority_id":self.authority_id,"workspace_id":self.workspace_id,"username":self.username})
    }
}
pub fn read_password(path: &Path) -> Result<String> {
    // One terminal LF/CRLF is a file delimiter; spaces are never silently stripped.
    let bytes = private_file::read(path, 4098)?;
    let mut password = String::from_utf8(bytes).map_err(|_| "invalid_password")?;
    if password.ends_with('\n') {
        password.pop();
        if password.ends_with('\r') {
            password.pop();
        }
    }
    if !(8..=4096).contains(&password.len()) || password.contains(['\n', '\r', '\0']) {
        return Err("invalid_password");
    }
    Ok(password)
}

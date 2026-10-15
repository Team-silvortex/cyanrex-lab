use super::{PlatformDatabaseConnectionError as Error, Result};
use std::{
    net::IpAddr,
    path::{Path, PathBuf},
};

enum Transport {
    Unix(PathBuf),
    Loopback(IpAddr),
}

/// Explicit local transport only. Syntax is not peer authentication or socket ownership proof.
pub struct PlatformDatabaseEndpoint {
    transport: Transport,
    port: u16,
}

impl PlatformDatabaseEndpoint {
    pub fn unix(directory: &Path, port: u16) -> Result<Self> {
        let text = directory.to_str().ok_or(Error::InvalidConfig)?;
        if port == 0
            || !text.starts_with('/')
            || text.chars().any(char::is_control)
            || text
                .split('/')
                .skip(1)
                .any(|part| part.is_empty() || matches!(part, "." | ".."))
            || format!("{text}/.s.PGSQL.{port}").len() > 100
        {
            return Err(Error::InvalidConfig);
        }
        Ok(Self {
            transport: Transport::Unix(directory.to_owned()),
            port,
        })
    }

    pub fn loopback(address: IpAddr, port: u16) -> Result<Self> {
        if !address.is_loopback() || port == 0 {
            return Err(Error::InvalidConfig);
        }
        Ok(Self {
            transport: Transport::Loopback(address),
            port,
        })
    }

    pub(super) fn apply(
        &self,
        options: sqlx_postgres::PgConnectOptions,
    ) -> sqlx_postgres::PgConnectOptions {
        let options = options.host("localhost").port(self.port);
        match &self.transport {
            Transport::Unix(path) => options.socket(path),
            Transport::Loopback(address) => options.host(&address.to_string()),
        }
    }
}

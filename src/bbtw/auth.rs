use color_eyre::{Result, eyre::ContextCompat};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use tokio::fs;
use tracing::instrument;

use crate::xdg_dirs;

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    pub created_at: OffsetDateTime,
    #[serde(serialize_with = "redact::expose_secret")]
    pub secret_token: redact::Secret<String>,
}

impl Token {
    /// Read an auth token from disk if it exists
    #[instrument]
    pub async fn read() -> Result<Option<Token>> {
        let path = token_path()?;
        tracing::debug!(?path, "Reading token");
        if path.exists() {
            let auth_token_str = fs::read_to_string(path).await?;
            let auth_token: Token = serde_json::from_str(&auth_token_str)?;
            Ok(Some(auth_token))
        } else {
            Ok(None)
        }
    }

    /// Write this token to disk.
    ///
    /// Writes to a file in the XDG state directory.
    pub async fn persist(&self, path: &Path) -> Result<()> {
        let token_str = serde_json::to_string(self)?;
        tracing::debug!(?path, "Writing token");

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, token_str).await?;

        Ok(())
    }
}

/// Return the path to the login token
///
/// It doesn't guarantee that it exists, it's just the path where it would be at.
pub fn token_path() -> Result<PathBuf> {
    let project_dir = xdg_dirs::new()?;
    Ok(project_dir
        .state_dir()
        .wrap_err("Missing XDG state dir")?
        .join("auth_token")
        .to_path_buf())
}

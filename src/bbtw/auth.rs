use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    Result,
    eyre::{Context, ContextCompat},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::fs;
use tracing::{debug, instrument};

use crate::{utils, xdg_dirs};

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    pub created_at: OffsetDateTime,
    #[serde(serialize_with = "redact::expose_secret")]
    pub secret_token: redact::Secret<String>,
}

impl Token {
    /// Read an auth token from disk if it exists
    #[instrument]
    pub async fn read(override_state_dir: Option<Utf8PathBuf>) -> Result<Option<Token>> {
        let path = token_path(override_state_dir)?;
        debug!(?path, "Reading token");
        if path.exists() {
            let auth_token_str = fs::read_to_string(path)
                .await
                .wrap_err("Could not read auth token")?;
            let auth_token: Token = serde_json::from_str(&auth_token_str)?;
            Ok(Some(auth_token))
        } else {
            Ok(None)
        }
    }

    /// Write this token to disk.
    ///
    /// Writes to a file in the XDG state directory.
    pub async fn persist(&self, path: &Utf8Path) -> Result<()> {
        let token_str = serde_json::to_string(self)?;
        debug!(?path, "Writing token");

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, token_str).await?;

        Ok(())
    }
}

/// Remove the token stored at the given path, if it exists.
///
/// Also succeeds if the file does not exist.
pub async fn delete_token(path: &Utf8Path) -> Result<()> {
    Ok(utils::remove_file_if_exists(path).await?)
}

/// Return the path to the login token
///
/// It doesn't guarantee that it exists, it's just the path where it would be at.
pub fn token_path(override_state_dir: Option<Utf8PathBuf>) -> Result<Utf8PathBuf> {
    let resolved_dir = if let Some(x) = override_state_dir {
        x
    } else {
        let project_dir = xdg_dirs::new()?;
        let state_dir = project_dir
            .state_dir()
            .wrap_err("Missing XDG state dir")?
            .to_path_buf();

        Utf8PathBuf::try_from(state_dir)?
    };

    Ok(resolved_dir.join("auth_token"))
}

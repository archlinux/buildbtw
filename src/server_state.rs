use camino::Utf8PathBuf;
use color_eyre::Result;
use redact::Secret;
use sea_orm::DatabaseConnection;
use tokio::sync::mpsc;
use url::Url;
use uuid::Uuid;

use crate::{iteration_creator, oidc};

/// Global shared state for axum handlers
#[derive(Clone, Debug)]
pub struct ServerState {
    /// SQLite connection for storing things on disk
    pub db: DatabaseConnection,

    /// Client configuration for logging in using a third-party OIDC provider
    pub oidc: Option<oidc::State>,

    /// Used to encrypt values stored as cookies in user's browsers
    pub cookie_encryption_key: Secret<axum_extra::extract::cookie::Key>,

    /// Override data storage dir used for package repos, build artifacts etc
    pub data_dir: Option<Utf8PathBuf>,

    /// URL the backend server is reachable at, including protocol.
    ///
    /// Port can be omitted if it's the standard port.
    /// E.g. <https://buildbtw.archlinux.org>
    pub server_url: Url,

    /// Optional channel for sending messages to the iteration creator.
    /// If the iteration creator is not running, this will be `None`.
    pub iteration_creator_message_sender: Option<mpsc::Sender<iteration_creator::Message>>,
}

impl ServerState {
    /// Notify the iteration creator about a new buildspace.
    /// If the iteration creator is not initialized, does nothing.
    pub async fn notify_iteration_creator_buildspace_created(
        &self,
        buildspace_id: Uuid,
    ) -> Result<()> {
        if let Some(sender) = self.iteration_creator_message_sender.as_ref() {
            sender
                .send(iteration_creator::Message::BuildspaceCreated { buildspace_id })
                .await?;
        }

        Ok(())
    }
}

/// Allows us to use the [axum_extra::extract::cookie::PrivateCookieJar]
/// extractor without explicitly passing the key.
impl axum::extract::FromRef<ServerState> for axum_extra::extract::cookie::Key {
    fn from_ref(state: &ServerState) -> Self {
        state.cookie_encryption_key.expose_secret().clone()
    }
}

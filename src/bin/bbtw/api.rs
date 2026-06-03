use camino::Utf8PathBuf;
use color_eyre::{Result, eyre::ContextCompat};
use url::Url;

pub mod builds;
pub mod user;

/// Configuration and state for accessing the buildbtw API over HTTP.
///
/// Used by functions in this module to dispatch requests.
pub struct Client {
    reqwest_client: reqwest::Client,
    buildbtw_server_url: Url,
}

impl Client {
    pub async fn new(
        buildbtw_server_url: Url,
        override_state_dir: Option<Utf8PathBuf>,
    ) -> Result<Client> {
        // Get auth token
        let auth_token = buildbtw::bbtw::auth::Token::read(override_state_dir)
            .await?
            .wrap_err("Please log in first.")?;
        let auth_token = auth_token.secret_token.expose_secret();

        // Put token into a sensitive header value
        let header_value = format!("Bearer {auth_token}");
        let mut bearer = reqwest::header::HeaderValue::from_str(&header_value)?;
        bearer.set_sensitive(true);

        // Create default header map
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, bearer);

        // Return client with default headers
        let reqwest_client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Client {
            reqwest_client,
            buildbtw_server_url,
        })
    }
}

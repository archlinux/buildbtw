use color_eyre::{Result, eyre::ContextCompat};

use crate::auth;

pub async fn new() -> Result<reqwest::Client> {
    // Get auth token
    let auth_token = auth::auth_token().await?.wrap_err("Please log in first.")?;
    let auth_token = auth_token.secret_token.expose_secret();

    // Put token into a sensitive header value
    let header_value = format!("Bearer {auth_token}");
    let mut bearer = reqwest::header::HeaderValue::from_str(&header_value)?;
    bearer.set_sensitive(true);

    // Create default header map
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, bearer);

    // Return client with default headers
    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .build()?)
}

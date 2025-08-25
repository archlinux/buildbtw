use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use buildbtw::web;
use color_eyre::Result;
use color_eyre::eyre::{Context, ContextCompat, OptionExt, eyre};
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreProviderMetadata, CoreUserInfoClaims,
};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, RedirectUrl, TokenResponse,
};
use openidconnect::{PkceCodeVerifier, reqwest};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::args;

/// State used by the http endpoints to run OIDC functionality.
/// Stored in [super::ServerState].
#[derive(Clone, Debug)]
#[expect(clippy::large_enum_variant)]
pub enum MaybeConfig {
    /// OIDC is either not configured at all, or the initialization failed.
    NotConfigured,
    /// An OIDC provider is configured and the server was able to connect to it
    /// at startup.
    Configured(Config),
}

impl MaybeConfig {
    /// Convenience function for turning [MaybeConfig] into a
    /// [`Result<Config>`].
    pub fn get_config(self) -> Result<Config> {
        match self {
            MaybeConfig::NotConfigured => Err(eyre!("OIDC client not configured")),
            MaybeConfig::Configured(config) => Ok(config),
        }
    }

    /// Initialize the OIDC configuration with the given command-line arguments.
    /// On failure, return [MaybeConfig::NotConfigured].
    pub async fn initialize(base_url: &Url, args: Option<args::Oidc>) -> MaybeConfig {
        match Self::try_initialize_state(base_url, args).await {
            Ok(conf) => {
                tracing::info!("OIDC enabled.");
                MaybeConfig::Configured(conf)
            }
            Err(e) => {
                tracing::info!("OIDC disabled: {e:?}");
                MaybeConfig::NotConfigured
            }
        }
    }

    /// Try to initialize an OIDC client for the given command-line arguments.
    async fn try_initialize_state(base_url: &Url, args: Option<args::Oidc>) -> Result<Config> {
        #[allow(unused_mut)]
        let mut reqwest_client_builder = openidconnect::reqwest::ClientBuilder::new()
            .redirect(openidconnect::reqwest::redirect::Policy::none());

        // Since we use self-signed certificates in tests, we need to make the reqwest
        // client accept them.
        #[cfg(test)]
        {
            let cert_bytes = std::fs::read("authelia/certificate.pem")?;
            let cert = reqwest::Certificate::from_pem(&cert_bytes)?;
            reqwest_client_builder = reqwest_client_builder
                .add_root_certificate(cert)
                // Seems like `add_root_certificate` is broken for both rustls and
                // native TLS: https://github.com/seanmonstar/reqwest/issues/1554
                // https://github.com/seanmonstar/reqwest/issues/1260
                // ಠ╭╮ಠ
                .danger_accept_invalid_certs(true);
        }
        let reqwest_client = reqwest_client_builder
            .build()
            .wrap_err("Failed to build reqwest client")?;

        let args = args.wrap_err("OIDC configuration is absent or incomplete.")?;
        let client_id = ClientId::new(args.oidc_client_id);
        let client_secret = ClientSecret::new(args.oidc_client_secret);
        let issuer_url =
            IssuerUrl::new(args.oidc_issuer_url).wrap_err("failed to parse issuer URL")?;

        // Query the provider for metadata
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &reqwest_client)
            .await
            .context("failed to discover provider")?;

        // Create the openidconnect client.
        let redirect_url = base_url.join(&web::oidc::Authorized {}.to_string())?;
        let client =
            CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
                .set_redirect_uri(RedirectUrl::from_url(redirect_url));

        Ok(Config {
            oidc_client: client,
            reqwest_client,
            issuer_name: args.oidc_issuer_name,
        })
    }
}

/// Everything needed at runtime to perform single-sign-on with a specific OIDC
/// provider.
#[derive(Clone, Debug)]
pub struct Config {
    /// High-level client from [openidconnect]
    pub oidc_client: ConfiguredClient,
    /// HTTP client passed to [openidconnect] functions when making requests
    pub reqwest_client: reqwest::Client,
    /// User-visible name of the OIDC provider ("issuer")
    pub issuer_name: String,
}

/// Used to store a valid and ready-to-use client in [Config].
pub type ConfiguredClient = CoreClient<
    openidconnect::EndpointSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointMaybeSet,
    openidconnect::EndpointMaybeSet,
>;

/// Start the OIDC login process. Return an URL for the user to visit, which
/// will subsequently redirect them back to our
/// [buildbtw::web::oidc::Authorized] endpoint.
/// Additionally, return a [LoginAttempt] struct which is the state we need to
/// store, and subsequently use to verify the authorization code in
/// [authorization_code_received].
pub async fn start_login(Config { oidc_client, .. }: Config) -> Result<(Url, LoginAttempt)> {
    // Generate a PKCE challenge.
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the full authorization URL.
    let (authorize_url, csrf_token, nonce) = oidc_client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        // Set the PKCE code challenge.
        .set_pkce_challenge(pkce_challenge)
        .url();

    Ok((
        authorize_url,
        LoginAttempt {
            nonce,
            csrf_token,
            pkce_verifier,
        },
    ))
}

#[derive(Serialize, Deserialize)]
pub struct LoginAttempt {
    /// Prevents replay attacks
    pub nonce: Nonce,
    /// Prevents CSRF attacks
    pub csrf_token: CsrfToken,
    /// Prevents CSRF and authorization code injection attacks
    pub pkce_verifier: PkceCodeVerifier,
}

pub const LOGIN_ATTEMPT_COOKIE_NAME: &str = "oidc_login_attempt";

impl LoginAttempt {
    pub fn from_cookie_jar(jar: PrivateCookieJar) -> Result<Self> {
        let cookie = jar
            .get(LOGIN_ATTEMPT_COOKIE_NAME)
            .wrap_err("Cookie not found")?;
        Ok(serde_json::from_str(cookie.value())?)
    }

    pub fn save_in_cookie_jar(&self, jar: PrivateCookieJar) -> Result<PrivateCookieJar> {
        let mut cookie = Cookie::new(LOGIN_ATTEMPT_COOKIE_NAME, serde_json::to_string(&self)?);
        cookie.set_same_site(SameSite::Strict);
        // TODO: serve the backend using TLS and enable the "Secure" flag
        // cookie.set_secure(true);
        cookie.set_http_only(true);
        let jar = jar.add(cookie);

        Ok(jar)
    }
}

/// Once the user has authorized the initial request, they are redirected to
/// [buildbtw::web::oidc::Authorized] with an authorization code in the query
/// string which allows us to obtain an ID token from the OIDC provider.
pub async fn authorization_code_received(
    Config {
        oidc_client,
        reqwest_client,
        ..
    }: Config,
    LoginAttempt {
        nonce,
        csrf_token: stored_csrf_token,
        pkce_verifier,
    }: LoginAttempt,
    authorization_code: AuthorizationCode,
    received_csrf_token: CsrfToken,
) -> Result<CoreUserInfoClaims> {
    if stored_csrf_token.secret() != received_csrf_token.secret() {
        return Err(eyre!("CSRF token mismatch"));
    }

    // Exchange the authorization code for an access token and ID token.
    let token_response = oidc_client
        .exchange_code(authorization_code)?
        // Set the PKCE code verifier.
        .set_pkce_verifier(pkce_verifier)
        .request_async(&reqwest_client)
        .await?;

    // Extract the ID token claims after verifying its authenticity and nonce.
    let id_token = token_response
        .id_token()
        .ok_or_eyre("Server did not return an ID token")?;
    let id_token_verifier = oidc_client.id_token_verifier();
    let claims = id_token.claims(&id_token_verifier, &nonce)?;

    // Verify the access token hash to ensure that the access token hasn't been
    // substituted for another user's.
    if let Some(expected_access_token_hash) = claims.access_token_hash() {
        let actual_access_token_hash = AccessTokenHash::from_token(
            token_response.access_token(),
            id_token.signing_alg()?,
            id_token.signing_key(&id_token_verifier)?,
        )?;
        if actual_access_token_hash != *expected_access_token_hash {
            return Err(eyre!("Invalid access token"));
        }
    }

    // Use the user info endpoint to request additional information.
    let userinfo: CoreUserInfoClaims = oidc_client
        .user_info(token_response.access_token().to_owned(), None)?
        .request_async(&reqwest_client)
        .await
        .wrap_err("Failed requesting user info")?;

    Ok(userinfo)
}

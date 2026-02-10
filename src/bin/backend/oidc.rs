//! Single-sign-on functionality using the Open ID Connect (OIDC) standard
//!
//! Overview:
//! When the server starts, [`MaybeConfig`] is initialized with arguments from
//! [args::Oidc]. If the OIDC provider is reachable and the configured
//! credentials are valid, [MaybeConfig::Configured] is stored in
//! [crate::ServerState].
//! Then, when a user visits [buildbtw::web::oidc::StartLogin], a [LoginAttempt]
//! is created and stored in their session. The user is redirected to the OIDC
//! provider to authorize our OIDC client for their account. Afterwards, they
//! are redirected back to [buildbtw::web::oidc::Authorized], and the
//! LoginAttempt is read from the session and used to validate the authorization
//! code. If everything is valid, the user is marked as logged in via their
//! session.

use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use buildbtw::web;
use color_eyre::Result;
use color_eyre::eyre::{Context, ContextCompat, OptionExt, bail, eyre};
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreGenderClaim, CoreProviderMetadata,
};
use openidconnect::{
    AccessTokenHash, AdditionalClaims, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, RedirectUrl, RefreshToken, Scope,
    TokenResponse, UserInfoClaims,
};
use openidconnect::{PkceCodeVerifier, reqwest};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{args, entities};

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
                tracing::info!("OIDC enabled");
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
        let mut reqwest_client_builder =
            reqwest::ClientBuilder::new().redirect(reqwest::redirect::Policy::none());

        // Since we use self-signed certificates in tests, we need to make the reqwest
        // client accept them.
        #[cfg(any(test, debug_assertions))]
        {
            tracing::info!("Danger: Allowing invalid TLS certs");
            reqwest_client_builder = reqwest_client_builder
                // .add_root_certificate(todo!())
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
        let client_secret = ClientSecret::new(args.oidc_client_secret.expose_secret().clone());
        let issuer_url =
            IssuerUrl::new(args.oidc_issuer_url).wrap_err("failed to parse issuer URL")?;

        // Query the provider for metadata
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &reqwest_client)
            .await
            .context("failed to discover provider")?;

        // Create the OIDC client.
        let redirect_url = base_url.join(&web::oidc::Authorized {}.to_string())?;
        let client =
            CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
                .set_redirect_uri(RedirectUrl::from_url(redirect_url));

        Ok(Config {
            oidc_client: client,
            reqwest_client,
            issuer_name: args.oidc_issuer_name,
            admin_oidc_groups: args.oidc_admin_groups,
            package_maintainer_oidc_groups: args.oidc_package_maintainer_groups,
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
    /// Users in one these OIDC groups will be assigned the "package maintainer" role.
    pub package_maintainer_oidc_groups: Vec<String>,
    /// Users in one these OIDC groups will be assigned the "admin" role. Takes precedence over other roles.
    pub admin_oidc_groups: Vec<String>,
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
/// [convert_authorization_code_to_user_info].
pub fn new_login_attempt(Config { oidc_client, .. }: Config) -> (Url, LoginAttempt) {
    // Generate a PKCE challenge.
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the full authorization URL.
    let (authorize_url, csrf_token, nonce) = oidc_client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        // See <https://openid.net/specs/openid-connect-core-1_0.html#UserInfo>, "5.4 Requesting Claims using Scope values"
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("groups".to_string()))
        // For receiving a refresh token, used to query user's groups in the background
        .add_scope(Scope::new("offline_access".to_string()))
        // Set the PKCE code challenge.
        .set_pkce_challenge(pkce_challenge)
        .url();

    (
        authorize_url,
        LoginAttempt {
            nonce,
            csrf_token,
            pkce_verifier,
        },
    )
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

pub const LOGIN_ATTEMPT_COOKIE_NAME: &str = "buildbtw_oidc_login_attempt";

impl LoginAttempt {
    pub fn from_cookie_jar(jar: &PrivateCookieJar) -> Result<Self> {
        let cookie = jar
            .get(LOGIN_ATTEMPT_COOKIE_NAME)
            .wrap_err("Cookie not found")?;
        Ok(serde_json::from_str(cookie.value())?)
    }

    pub fn save_in_cookie_jar(&self, jar: PrivateCookieJar) -> Result<PrivateCookieJar> {
        let mut cookie = Cookie::new(LOGIN_ATTEMPT_COOKIE_NAME, serde_json::to_string(&self)?);
        cookie.set_same_site(SameSite::Strict);
        // TODO: serve the backend using TLS and enable the "Secure" flag
        // https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/190
        // cookie.set_secure(true);
        cookie.set_http_only(true);
        let jar = jar.add(cookie);

        Ok(jar)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupClaims {
    groups: Vec<String>,
}

impl AdditionalClaims for GroupClaims {}

/// Once the user has authorized the initial request, they are redirected to
/// [buildbtw::web::oidc::Authorized] with an authorization code in the query
/// string which allows us to obtain an ID token from the OIDC provider.
///
/// Returns the user info claims and an optional refresh token.
pub async fn convert_authorization_code_to_user_info(
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
) -> Result<(
    UserInfoClaims<GroupClaims, CoreGenderClaim>,
    Option<RefreshToken>,
)> {
    if stored_csrf_token != received_csrf_token {
        bail!("CSRF token mismatch");
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
            bail!("Invalid access token");
        }
    }

    // Use the user info endpoint to request additional information.
    let userinfo = oidc_client
        .user_info(token_response.access_token().to_owned(), None)?
        .request_async(&reqwest_client)
        .await
        .wrap_err("Failed requesting user info")?;

    // Extract the refresh token if present
    let refresh_token = token_response.refresh_token().cloned();

    Ok((userinfo, refresh_token))
}

pub fn oidc_groups_to_user_roles(
    user_groups: &GroupClaims,
    admin_group_names: &[String],
    package_maintainer_group_names: &[String],
) -> Vec<entities::user_roles::Role> {
    let mut roles = Vec::new();

    let is_admin = admin_group_names
        .iter()
        .any(|group_name| user_groups.groups.contains(group_name));

    if is_admin {
        roles.push(entities::user_roles::Role::Admin);
    }

    let is_package_maintainer = package_maintainer_group_names
        .iter()
        .any(|group_name| user_groups.groups.contains(group_name));

    if is_package_maintainer {
        roles.push(entities::user_roles::Role::PackageMaintainer);
    }

    roles
}

/// Fetch fresh user info from OIDC provider using a refresh token.
///
/// If the provider uses refresh token rotation, it returns a new
/// refresh token that should replace the old one.
pub async fn fetch_user_info_with_refresh_token(
    Config {
        oidc_client,
        reqwest_client,
        ..
    }: &Config,
    refresh_token: String,
) -> Result<(
    UserInfoClaims<GroupClaims, CoreGenderClaim>,
    Option<RefreshToken>,
)> {
    // Exchange refresh token for a new access token
    let token_response = oidc_client
        .exchange_refresh_token(&RefreshToken::new(refresh_token))?
        .request_async(reqwest_client)
        .await
        .wrap_err("Failed to exchange refresh token")?;

    // Extract the new refresh token if present (for refresh token rotation)
    let new_refresh_token = token_response.refresh_token().cloned();

    // Use the new access token to fetch user info
    let user_info = oidc_client
        .user_info(token_response.access_token().to_owned(), None)?
        .request_async(reqwest_client)
        .await
        .wrap_err("Failed requesting user info with refreshed token")?;

    Ok((user_info, new_refresh_token))
}

#[cfg(test)]
mod tests {
    use crate::{
        entities::user_roles::Role,
        oidc::{GroupClaims, oidc_groups_to_user_roles},
    };

    #[test]
    fn test_oidc_groups_to_user_roles() {
        // Test admin matching
        let roles = oidc_groups_to_user_roles(
            &GroupClaims {
                groups: vec!["Admin Group".to_string()],
            },
            &["Other Group".to_string(), "Admin Group".to_string()],
            &["Package Maintainer Group".to_string()],
        );
        assert_eq!(roles, vec![Role::Admin]);

        // Test missing match returning empty vec
        let roles = oidc_groups_to_user_roles(
            &GroupClaims {
                groups: vec!["Normal Group".to_string()],
            },
            &["Other Group".to_string(), "Admin Group".to_string()],
            &["Package Maintainer Group".to_string()],
        );
        assert_eq!(roles, Vec::<Role>::new());

        // Test package maintainer
        let roles = oidc_groups_to_user_roles(
            &GroupClaims {
                groups: vec!["Package Maintainer Group".to_string()],
            },
            &["Other Group".to_string(), "Admin Group".to_string()],
            &["Package Maintainer Group".to_string()],
        );
        assert_eq!(roles, vec![Role::PackageMaintainer]);

        // Test user in both admin and maintainer groups gets both roles
        let roles = oidc_groups_to_user_roles(
            &GroupClaims {
                groups: vec![
                    "Admin Group".to_string(),
                    "Package Maintainer Group".to_string(),
                ],
            },
            &["Admin Group".to_string()],
            &["Package Maintainer Group".to_string()],
        );
        assert_eq!(roles, vec![Role::Admin, Role::PackageMaintainer]);

        // Test empty user & admin groups
        let roles = oidc_groups_to_user_roles(&GroupClaims { groups: vec![] }, &[], &[]);
        assert_eq!(roles, Vec::<Role>::new());
    }
}

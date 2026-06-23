//! Single-sign-on functionality using the Open ID Connect (OIDC) standard
//!
//! Overview:
//! When the server starts, [`State`] is initialized with an [`InitConfig`]. If the OIDC provider is reachable and the configured
//! credentials are valid, [`State`] is stored in
//! [`crate::server_state::ServerState`].
//! Then, when a user visits [crate::web::oidc::StartLogin], a [LoginAttempt]
//! is created and stored in their session. The user is redirected to the OIDC
//! provider to authorize our OIDC client for their account. Afterwards, they
//! are redirected back to [crate::web::oidc::Authorized], and the
//! LoginAttempt is read from the session and used to validate the authorization
//! code. If everything is valid, the user is marked as logged in via their
//! session.

use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use color_eyre::{
    Result,
    eyre::{Context, ContextCompat, OptionExt, bail},
};
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreGenderClaim, CoreProviderMetadata,
};
use openidconnect::{
    AccessTokenHash, AdditionalClaims, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, RedirectUrl, RefreshToken, Scope,
    TokenResponse, UserInfoClaims,
};
use openidconnect::{PkceCodeVerifier, reqwest};
use redact::Secret;
use serde::{Deserialize, Serialize};
use tracing::info;
use url::Url;

use crate::web;
use crate::{db_fields::RedactedString, entities};

/// OIDC configuration for initializing the client.
#[derive(Debug)]
pub struct InitConfig {
    pub client_id: String,
    pub client_secret: Secret<String>,
    pub issuer_url: IssuerUrl,
    pub issuer_name: String,
    pub package_maintainer_groups: Vec<String>,
    pub admin_groups: Vec<String>,
}

/// Everything needed at runtime to perform single-sign-on with a specific OIDC
/// provider.
/// Stored in [crate::server_state::ServerState].
#[derive(Clone, Debug)]
pub struct State {
    /// High-level client from [openidconnect]
    pub oidc_client: ConfiguredClient,

    /// HTTP client passed to [openidconnect] functions when making requests
    pub reqwest_client: reqwest::Client,

    /// User-visible name of the OIDC provider ("issuer")
    pub issuer_name: String,

    /// Url of the OIDC provider ("issuer")
    pub issuer_url: IssuerUrl,

    /// Users in one these OIDC groups will be assigned the "package maintainer" role.
    pub package_maintainer_oidc_groups: Vec<String>,

    /// Users in one these OIDC groups will be assigned the "admin" role. Takes precedence over other roles.
    pub admin_oidc_groups: Vec<String>,
}

impl State {
    /// Initialize the OIDC state
    pub async fn initialize(server_url: &Url, config: InitConfig) -> Result<Self> {
        #[allow(unused_mut)]
        let mut reqwest_client_builder =
            reqwest::ClientBuilder::new().redirect(reqwest::redirect::Policy::none());

        // Since we use self-signed certificates in tests, we need to make the reqwest
        // client accept them.
        #[cfg(any(test, debug_assertions))]
        {
            tracing::warn!("Danger: Allowing invalid TLS certs");
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

        let client_id = ClientId::new(config.client_id);
        let client_secret = ClientSecret::new(config.client_secret.expose_secret().clone());

        // Query the provider for metadata
        let provider_metadata =
            CoreProviderMetadata::discover_async(config.issuer_url.clone(), &reqwest_client)
                .await
                .wrap_err("failed to discover provider")?;

        // Create the OIDC client.
        let redirect_url = server_url.join(&web::oidc::Authorized {}.to_string())?;
        let client =
            CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
                .set_redirect_uri(RedirectUrl::from_url(redirect_url));

        info!("OIDC enabled");
        Ok(State {
            oidc_client: client,
            reqwest_client,
            issuer_name: config.issuer_name,
            issuer_url: config.issuer_url,
            admin_oidc_groups: config.admin_groups,
            package_maintainer_oidc_groups: config.package_maintainer_groups,
        })
    }
}

/// Used to store a valid and ready-to-use client in [State].
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
/// [crate::web::oidc::Authorized] endpoint.
/// Additionally, return a [LoginAttempt] struct which is the state we need to
/// store, and subsequently use to verify the authorization code in
/// [convert_authorization_code_to_user_info].
pub fn new_login_attempt(State { oidc_client, .. }: State) -> (Url, LoginAttempt) {
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

#[derive(Debug, Serialize, Deserialize)]
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

    pub fn save_in_cookie_jar(
        &self,
        jar: PrivateCookieJar,
        oidc_config: &State,
    ) -> Result<PrivateCookieJar> {
        let mut cookie = Cookie::new(LOGIN_ATTEMPT_COOKIE_NAME, serde_json::to_string(&self)?);
        cookie.set_same_site(same_site_from_oidc_config(Some(oidc_config)));
        cookie.set_secure(true);
        cookie.set_http_only(true);
        let jar = jar.add(cookie);

        Ok(jar)
    }

    pub fn remove_from_cookie_jar(cookie_jar: PrivateCookieJar) -> PrivateCookieJar {
        let mut cookie = Cookie::from(LOGIN_ATTEMPT_COOKIE_NAME);
        cookie.set_http_only(true);
        cookie.set_secure(true);
        cookie_jar.remove(cookie)
    }
}

/// Extracts up til the second-level domain and discards any subdomains.
fn second_level_domain(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    let mut it = host.rsplitn(3, '.');
    let tld = it.next()?;
    let sld = it.next()?;
    Some(format!("{sld}.{tld}"))
}

/// Determines cookie same-site settings depending on if the oidc issuer and the target redirect uri
/// are cross-origin or share the same parent.
///
/// Returns lax on cross-origin domains, strict in all other cases including no oidc.
#[must_use]
pub fn same_site_from_oidc_config(oidc_config: Option<&State>) -> SameSite {
    let Some(oidc_config) = oidc_config else {
        return SameSite::Strict;
    };
    let Some(redirect_uri) = oidc_config.oidc_client.redirect_uri() else {
        return SameSite::Strict;
    };
    let Some(redirect_domain) = second_level_domain(redirect_uri.url()) else {
        return SameSite::Strict;
    };
    let Some(oidc_domain) = second_level_domain(&oidc_config.issuer_url.clone().into()) else {
        return SameSite::Strict;
    };

    // Use strict same-site cookies for matching second-level domain sharing a parent.
    // It is required to set lax in case of diverging domains, otherwise the OIDC cookie
    // cannot be accessed in a cross-origin redirect flow.
    if redirect_domain != oidc_domain {
        return SameSite::Lax;
    }
    SameSite::Strict
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GroupClaims {
    groups: Vec<String>,
}

impl AdditionalClaims for GroupClaims {}

/// Once the user has authorized the initial request, they are redirected to
/// [crate::web::oidc::Authorized] with an authorization code in the query
/// string which allows us to obtain an ID token from the OIDC provider.
///
/// Returns the user info claims and an optional refresh token.
pub async fn convert_authorization_code_to_user_info(
    State {
        oidc_client,
        reqwest_client,
        ..
    }: State,
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

#[must_use]
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
    State {
        oidc_client,
        reqwest_client,
        ..
    }: &State,
    refresh_token: RedactedString,
) -> Result<(
    UserInfoClaims<GroupClaims, CoreGenderClaim>,
    Option<RefreshToken>,
)> {
    // Exchange refresh token for a new access token
    let token_response = oidc_client
        .exchange_refresh_token(&RefreshToken::new(
            refresh_token.expose_secret().to_string(),
        ))?
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
    use rstest::rstest;
    use url::Url;

    use crate::{
        entities::user_roles::Role,
        oidc::{GroupClaims, oidc_groups_to_user_roles, second_level_domain},
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

    #[rstest]
    #[case("https://archlinux.org", Some("archlinux.org"))]
    #[should_panic(expected = "assertion")]
    #[case("https://archlinux.co.uk", Some("archlinux.co.uk"))]
    #[case("https://foo.archlinux.org", Some("archlinux.org"))]
    #[case("https://foo.bar.archlinux.org", Some("archlinux.org"))]
    #[case("https://archlinux.org/foo?bar=42", Some("archlinux.org"))]
    #[case("https://localhost", None)]
    fn test_second_level_domain(#[case] input: &str, #[case] expected: Option<&str>) {
        assert_eq!(
            second_level_domain(&Url::parse(input).unwrap()),
            expected.map(String::from)
        );
    }
}

use color_eyre::Result;
use color_eyre::eyre::{Context, OptionExt, eyre};
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreProviderMetadata, CoreUserInfoClaims,
};
use openidconnect::reqwest;
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, RedirectUrl, Scope, TokenResponse,
};

/// State used by the http endpoints to run OIDC functionality.
/// Stored in [ServerState].
#[derive(Clone)]
#[expect(clippy::large_enum_variant)]
pub enum State {
    NotConfigured,
    Configured(Config),
}

#[derive(Clone)]
pub struct Config {
    pub client: ConfiguredClient,
    pub reqwest_client: reqwest::Client,
    pub name: String,
}

/// Used to store a valid and ready-to-use client in [Config].
pub type ConfiguredClient = CoreClient;

async fn login() -> Result<CoreUserInfoClaims> {
    let http_client = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    // Use OpenID Connect Discovery to fetch the provider metadata.
    let provider_metadata = CoreProviderMetadata::discover_async(
        IssuerUrl::new("https://accounts.example.com".to_string())?,
        &http_client,
    )
    .await?;

    // Create an OpenID Connect client by specifying the client ID, client secret,
    // authorization URL and token URL.
    let client = CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new("client_id".to_string()),
        Some(ClientSecret::new("client_secret".to_string())),
    )
    // Set the URL the user will be redirected to after the authorization process.
    .set_redirect_uri(RedirectUrl::new("http://redirect".to_string())?);

    // Generate a PKCE challenge.
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the full authorization URL.
    let (auth_url, csrf_token, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        // Set the desired scopes.
        .add_scope(Scope::new("read".to_string()))
        .add_scope(Scope::new("write".to_string()))
        // Set the PKCE code challenge.
        .set_pkce_challenge(pkce_challenge)
        .url();

    // This is the URL you should redirect the user to, in order to trigger the
    // authorization process.
    println!("Browse to: {}", auth_url);

    // Once the user has been redirected to the redirect URL, you'll have access to
    // the authorization code. For security reasons, your code should verify
    // that the `state` parameter returned by the server matches `csrf_state`.

    // Now you can exchange it for an access token and ID token.
    let token_response = client
        .exchange_code(AuthorizationCode::new(
            "some authorization code".to_string(),
        ))?
        // Set the PKCE code verifier.
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client)
        .await?;

    // Extract the ID token claims after verifying its authenticity and nonce.
    let id_token = token_response
        .id_token()
        .ok_or_eyre("Server did not return an ID token")?;
    let id_token_verifier = client.id_token_verifier();
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

    // The authenticated user's identity is now available. See the IdTokenClaims
    // struct for a complete listing of the available claims.
    println!(
        "User {} with e-mail address {} has authenticated successfully",
        claims.subject().as_str(),
        claims
            .email()
            .map(|email| email.as_str())
            .unwrap_or("<not provided>"),
    );

    // If available, we can use the user info endpoint to request additional
    // information.

    // The user_info request uses the AccessToken returned in the token response. To
    // parse custom claims, use UserInfoClaims directly (with the desired type
    // parameters) rather than using the CoreUserInfoClaims type alias.
    let userinfo: CoreUserInfoClaims = client
        .user_info(token_response.access_token().to_owned(), None)?
        .request_async(&http_client)
        .await
        .wrap_err("Failed requesting user info")?;

    // See the OAuth2TokenResponse trait for a listing of other available fields
    // such as access_token() and refresh_token().

    Ok(userinfo)
}

//! Routes and parameters for single-sign-on functionality via Open ID Connect
use axum_extra::routing::TypedPath;
use openidconnect::{AuthorizationCode, CsrfToken};
use serde::Deserialize;

/// Start the login process via the globally configured OIDC provider.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/oidc/start_login")]
pub struct StartLogin {}

/// Query parameters for the login start endpoint.
#[derive(Deserialize, Debug)]
pub struct StartLoginQuery {
    /// Optional URL to redirect to after successful login.
    pub next: Option<String>,
}

/// Once the user has authorized the request with the OIDC provider, they are
/// redirected here with an authorization code in the query string which allows
/// us to obtain an ID token from the OIDC provider.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/oidc/authorized")]
pub struct Authorized {}

/// Authorization data passed to us from the OIDC provider via query string.
#[derive(Deserialize, Debug)]
pub struct LoginRedirectQuery {
    /// Used to obtain an ID token from the OIDC provider
    pub code: AuthorizationCode,
    /// Used to prevent CSRF attacks
    pub state: CsrfToken,
}

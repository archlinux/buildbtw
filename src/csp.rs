//! Per-response Content-Security-Policy handling

use axum::extract::{FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use color_eyre::eyre::{OptionExt, WrapErr};
use uuid::Uuid;

use crate::response_error::{ResponseError, ResponseResult};
use crate::server_state::ServerState;

/// Strict Content Security Policy locking the browser to this origin
///
/// `{nonce}` is replaced by a random nonce per response.
///
/// See also: <https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CSP>
const CSP: &str = concat!(
    "default-src 'self'; script-src 'self' 'nonce-{nonce}'; style-src 'self'; img-src 'self'; ",
    "font-src 'self'; connect-src 'self'; base-uri 'self'; form-action 'self'; ",
    "object-src 'none'; frame-ancestors 'none'; ",
    "require-trusted-types-for 'script'; trusted-types datastar",
);

#[derive(Clone, Debug)]
pub struct Nonce(String);

impl Nonce {
    fn generate() -> Self {
        Self(Uuid::new_v4().simple().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Extractor for the [`Nonce`] of the current response
impl FromRequestParts<ServerState> for Nonce {
    type Rejection = ResponseError;

    async fn from_request_parts(
        parts: &mut Parts,
        _: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        let nonce = parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or_eyre("The CSP middleware did not run for this request")?;

        Ok(nonce)
    }
}

/// Give every request a fresh [`Nonce`] and put the resulting policy header into the response
pub async fn middleware(mut request: Request, next: Next) -> ResponseResult<Response> {
    let nonce = Nonce::generate();
    request.extensions_mut().insert(nonce.clone());

    let mut response = next.run(request).await;
    let policy = CSP.replace("{nonce}", nonce.as_str());
    let policy = HeaderValue::from_str(&policy).wrap_err("CSP must be a valid header value")?;
    response
        .headers_mut()
        .insert(header::CONTENT_SECURITY_POLICY, policy);

    Ok(response)
}

use axum::{
    RequestPartsExt,
    extract::{FromRequestParts, OptionalFromRequestParts},
    http::request::Parts,
};
use axum_extra::{
    TypedHeader,
    extract::{
        PrivateCookieJar,
        cookie::{Cookie, SameSite},
    },
    headers::{Authorization, authorization::Bearer},
};
use color_eyre::eyre::{Context, ContextCompat};
use sea_orm::{IntoActiveModel, ModelTrait};
use serde::Serialize;

use crate::{
    db, db_fields::RedactedString, entities, queries, response_error::ResponseError,
    server_state::ServerState,
};

pub const SESSION_SECRET_TOKEN_COOKIE_NAME: &str = "buildbtw_session_secret_token";

/// Holds authentication data for a logged-in user.
///
/// This struct bundles the active session and the corresponding user model
/// with eagerly loaded roles. It is passed to request handlers that require
/// authentication, allowing them to access both the session information
/// and the owning user's data with their roles.
#[derive(Clone, Debug, Serialize)]
pub struct AuthUser {
    pub session: entities::sessions::Model,
    pub user: entities::users::Model,
    pub roles: Vec<entities::user_roles::Role>,
}

/// Implements optional extraction of [`AuthUser`].
///
/// This allows request handlers to declare an `Option<AuthUser>` parameter
/// instead of requiring authentication. If the user is authenticated, the
/// extractor provides their session and user data; otherwise it returns
/// `None` without causing a rejection. This is useful for endpoints that
/// work for both authenticated and unauthenticated users.
///
/// Internally, this calls the standard [`FromRequestParts<ServerState>`]
/// implementation for [`AuthUser`] and converts its result into an `Option`
impl OptionalFromRequestParts<ServerState> for AuthUser {
    type Rejection = ResponseError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ServerState,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(
            <AuthUser as FromRequestParts<ServerState>>::from_request_parts(parts, state)
                .await
                .ok(),
        )
    }
}

/// Extractor for enforcing authentication on protected endpoints.
///
/// This implementation ensures that a valid authenticated user exists
/// before the request handler is executed. If the user is not authenticated,
/// the request is rejected with an appropriate error response.
///
/// When authentication succeeds, the user's session model is loaded and
/// its last access time is automatically updated in the database, ensuring
/// accurate session activity tracking.
impl FromRequestParts<ServerState> for AuthUser {
    type Rejection = ResponseError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        let db::Tx(tx) = db::Tx::from_request_parts(parts, state).await?;

        // First, we'll try to get a session secret token from the cookie.
        // This is the expected case for an interactive browser session.
        let cookie_jar: PrivateCookieJar = PrivateCookieJar::from_request_parts(parts, state)
            .await
            .wrap_err("Failed to extract cookie jar")?;
        let secret_token_from_cookie = cookie_jar.get(SESSION_SECRET_TOKEN_COOKIE_NAME);

        // Next, we'll attempt to get it from the authorization header as a bearer token.
        // This would be the case if the client is a CLI.
        let secret_token_from_header = if let Ok(TypedHeader(Authorization(bearer))) =
            parts.extract::<TypedHeader<Authorization<Bearer>>>().await
        {
            Some(bearer)
        } else {
            None
        };

        let secret_token = if let Some(secret_token_from_cookie) = secret_token_from_cookie {
            RedactedString::from(secret_token_from_cookie.value().to_string())
        } else if let Some(secret_token_from_header) = secret_token_from_header {
            RedactedString::from(secret_token_from_header.token().to_string())
        } else {
            return Err(ResponseError::NotAuthenticated);
        };

        let session_with_user = queries::sessions::by_secret_token(secret_token)
            .find_also_related(entities::users::Entity)
            .one(&tx)
            .await?;

        let Some((session, user)) = session_with_user else {
            // Session does not exist in the database
            return Err(ResponseError::NotAuthenticated);
        };

        // Can only happen on severe corruption, as the session has a foreign key on the user
        let user = user.wrap_err("Session does not have a user")?;

        let roles = user
            .find_related(entities::user_roles::Entity)
            .all(&tx)
            .await?
            .into_iter()
            .map(|model| model.role)
            .collect();

        let session = queries::sessions::update_last_accessed_time(session.into_active_model())
            .exec(&tx)
            .await?;
        tx.commit().await?;

        Ok(AuthUser {
            session,
            user,
            roles,
        })
    }
}

pub fn save_in_cookie_jar(
    session_secret_token: &RedactedString,
    cookie_jar: PrivateCookieJar,
) -> PrivateCookieJar {
    let mut cookie = Cookie::new(
        SESSION_SECRET_TOKEN_COOKIE_NAME,
        session_secret_token.expose_secret().to_owned(),
    );
    cookie.set_same_site(SameSite::Strict);
    cookie.set_path("/");
    cookie.set_http_only(true);
    // TODO: serve the backend using TLS and enable the "Secure" flag
    // https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/190
    // cookie.set_secure(true);
    cookie_jar.add(cookie)
}

pub fn remove_from_cookie_jar(cookie_jar: PrivateCookieJar) -> PrivateCookieJar {
    let mut cookie = Cookie::from(SESSION_SECRET_TOKEN_COOKIE_NAME);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_path("/");
    cookie.set_http_only(true);
    // TODO: serve the backend using TLS and enable the "Secure" flag
    // https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/190
    // cookie.set_secure(true);
    cookie_jar.remove(cookie)
}

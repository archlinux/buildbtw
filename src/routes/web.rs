//! Browser UI: used by humans through the browser

use axum::Router;
use axum::http::{HeaderValue, header};
use axum_extra::routing::RouterExt;
use camino::Utf8Path;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::server_state::ServerState;

mod account;
mod admin;
pub mod index;
mod oidc;

pub fn router(root: &Utf8Path) -> Router<ServerState> {
    // Set no-cache for the assets to browser will check cache validity themselves with a cheap header request.
    // See https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Cache-Control#no-cache_2
    let no_cache = SetResponseHeaderLayer::overriding(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    let static_files = ServeDir::new(format!("{root}/assets"));

    Router::new()
        .nest_service("/assets", static_files)
        .layer(no_cache)
        .typed_get(index::index)
        .typed_get(oidc::start_login)
        .typed_get(oidc::authorized)
        .typed_get(account::overview)
        .typed_post(account::logout)
        .typed_get(account::session_list)
        .typed_post(account::session_revoke)
        .typed_get(account::cli_session_landing)
        .typed_post(account::cli_session_create)
        .typed_get(admin::bot_list)
        .typed_post(admin::bot_create)
        .typed_post(admin::bot_token_regenerate)
}

//! Browser UI: used by humans through the browser

use axum::Router;
use axum_extra::routing::RouterExt;
use camino::Utf8PathBuf;
use tower_http::services::ServeDir;

use crate::server_state::ServerState;

mod account;
pub mod index;
mod oidc;

pub fn router(root: Utf8PathBuf) -> Router<ServerState> {
    let static_files = ServeDir::new(format!("{root}/assets"));

    Router::new()
        .nest_service("/assets", static_files)
        .typed_get(index::index)
        .typed_get(oidc::start_login)
        .typed_get(oidc::authorized)
        .typed_get(account::overview)
        .typed_get(account::logout)
        .typed_get(account::session_list)
        .typed_get(account::session_revoke)
}

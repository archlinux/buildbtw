//! Browser UI: used by humans through the browser

use axum::Router;
use axum_extra::routing::RouterExt;

use crate::server_state::ServerState;

mod account;
pub mod index;
mod oidc;

pub fn router() -> Router<ServerState> {
    Router::new()
        .typed_get(index::index)
        .typed_get(oidc::start_login)
        .typed_get(oidc::authorized)
        .typed_get(account::logout)
        .typed_get(account::session_list)
        .typed_get(account::session_revoke)
}

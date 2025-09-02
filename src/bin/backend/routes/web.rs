//! Browser UI: used by humans through the browser

use axum::Router;
use axum_extra::routing::RouterExt;

use crate::server_state::ServerState;

pub mod index;

pub fn router() -> Router<ServerState> {
    Router::new().typed_get(index::index)
}

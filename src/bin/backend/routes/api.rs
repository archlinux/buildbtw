//! JSON API: used by clients and other programs

use axum::Router;
use axum_extra::routing::RouterExt;

use crate::server_state::ServerState;

mod builds;

pub fn router() -> Router<ServerState> {
    Router::new()
        .typed_get(builds::list)
        .typed_post(builds::upload_package)
}

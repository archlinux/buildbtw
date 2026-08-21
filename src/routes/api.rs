//! JSON API: used by clients and other programs

use axum::Router;
use axum_extra::routing::RouterExt;

use crate::server_state::ServerState;

mod builds;
mod buildspaces;
mod health;
mod users;

pub fn router() -> Router<ServerState> {
    Router::new()
        .typed_get(builds::list)
        .typed_get(builds::download_package)
        .typed_get(builds::download_log)
        .typed_get(builds::serve_repo_file)
        .typed_put(builds::set_status)
        .typed_post(buildspaces::create)
        .typed_get(buildspaces::list)
        .typed_get(buildspaces::get_with_iteration)
        .typed_get(buildspaces::get_with_latest_iteration)
        .typed_put(buildspaces::set_status)
        .typed_get(users::user)
        .typed_post(users::create)
        .typed_get(health::health)
}

pub fn streaming_router() -> Router<ServerState> {
    Router::new()
        .typed_post(builds::upload_package)
        .typed_post(builds::upload_log)
}

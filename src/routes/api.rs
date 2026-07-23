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
        .typed_post(builds::upload_package)
        .typed_get(builds::download_package)
        .typed_get(builds::serve_repo_file)
        .typed_post(buildspaces::create)
        .typed_get(buildspaces::get)
        .typed_put(buildspaces::close)
        .typed_get(users::user)
        .typed_get(health::health)
}

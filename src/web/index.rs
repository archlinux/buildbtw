//! Routes and parameters for web pages interacting with
//! [crate::web::index::Index]

use axum_extra::routing::TypedPath;
use serde::Deserialize;

/// Show the start page
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/")]
pub struct Index {}

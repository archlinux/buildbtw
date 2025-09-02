//! Routes and parameters for web pages interacting with
//! [crate::api::builds::Build]

use axum_extra::routing::TypedPath;
use serde::Deserialize;

/// Show the start page
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/")]
pub struct Index {}

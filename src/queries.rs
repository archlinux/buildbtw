//! Functionality for interacting with the database, e.g. from web server
//! endpoints. By convention, we usually return query builders (e.g.
//! [sea_orm::Select]) from functions here, so callers can add pagination,
//! stream results, etc.
pub mod builds;
pub mod buildspaces;
pub mod gitlab_pipelines;
pub mod global_state;
pub mod iterations;
pub mod sessions;
pub mod user_roles;
pub mod users;

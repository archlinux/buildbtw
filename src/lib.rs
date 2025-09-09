//! The buildbtw library, providing functionality for all the binaries we
//! release.
pub mod api;
pub mod authelia_container;
pub mod tracing;
pub mod web;

pub use authelia_container::AutheliaContainer;

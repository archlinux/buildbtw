// [sea_orm::DeriveEntityModel] generates qualified references to some types
// so we'll allow this lint in this module to make life easier
#![expect(unused_qualifications)]

pub mod build_dependencies;
pub mod builds;
pub mod buildspaces;
pub mod global_state;
pub mod iterations;
pub mod oidc_identity;
pub mod sessions;
pub mod user_roles;
pub mod users;

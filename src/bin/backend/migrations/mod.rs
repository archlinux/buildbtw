//! Database migrations for the backend server.
//!
//! Contrary to our style guide, this file is named mod.rs instead of
//! migrations.rs due to the following seaORM issue:
//! <https://github.com/SeaQL/sea-orm/issues/2690>

use sea_orm_migration::prelude::*;

mod m20250811_165601_init;
mod m20250923_000000_add_users;
mod m20250925_173232_add_sessions;
mod m20251218_184700_add_user_roles;
mod m20260108_000000_add_user_refresh_tokens;
mod m20260224_130113_add_build_dependencies;
mod m20260224_131450_unique_builds;
mod m20260225_112639_remove_build_repository_name;
mod m20260301_032351_secret_session_token;

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250811_165601_init::Migration),
            Box::new(m20250923_000000_add_users::Migration),
            Box::new(m20250925_173232_add_sessions::Migration),
            Box::new(m20251218_184700_add_user_roles::Migration),
            Box::new(m20260108_000000_add_user_refresh_tokens::Migration),
            Box::new(m20260224_130113_add_build_dependencies::Migration),
            Box::new(m20260224_131450_unique_builds::Migration),
            Box::new(m20260225_112639_remove_build_repository_name::Migration),
            Box::new(m20260301_032351_secret_session_token::Migration),
        ]
    }
}

// [sea_orm::DeriveEntityModel] generates qualified references to some types
// so we'll allow this lint in this module to make life easier
#![allow(unused_qualifications)]

use color_eyre::eyre::Result;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::db::migrations::Migrator;

mod branch_name;
mod build;
mod build_status;
mod changesets;
mod concrete_architecture;
mod iteration;
mod migrations;
mod namespace;
mod new_iteration_reason;
mod pkgbase;
mod pkgname;
mod pkgnames;

/// Create the database at the given URL if it doesn't exist,
/// run any migrations that have not run yet, and return a connection to the
/// database.
pub async fn create_migrate_connect(db_url: redact::Secret<String>) -> Result<DatabaseConnection> {
    let db = Database::connect(db_url.expose_secret()).await?;
    Migrator::up(&db, None).await?;

    Ok(db)
}

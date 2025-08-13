// [sea_orm::DeriveEntityModel] generates qualified references to some types
// so we'll allow this lint in this module to make life easier
#![allow(unused_qualifications)]

use camino::Utf8Path;
use color_eyre::eyre::Result;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

pub use crate::migrations::Migrator;

mod branch_name;
mod build;
mod build_status;
mod changesets;
mod concrete_architecture;
mod iteration;
mod namespace;
mod new_iteration_reason;
mod pkgbase;
mod pkgname;
mod pkgnames;
mod repository_name;
mod version;

/// Create the database at the given URL if it doesn't exist,
/// run any migrations that have not run yet, and return a connection to the
/// database.
pub async fn create_migrate_connect(db_file: &Utf8Path) -> Result<DatabaseConnection> {
    let db_url = format!("sqlite://{db_file}?mode=rwc");
    let db = Database::connect(db_url).await?;
    Migrator::up(&db, None).await?;

    Ok(db)
}

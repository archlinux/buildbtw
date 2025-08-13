// [sea_orm::DeriveEntityModel] generates qualified references to some types
// so we'll allow this lint in this module to make life easier
#![allow(unused_qualifications)]

use camino::Utf8Path;
use color_eyre::eyre::Result;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

pub use crate::migrations::Migrator;

pub mod branch_name;
pub mod build;
pub mod build_status;
pub mod changesets;
pub mod concrete_architecture;
pub mod iteration;
pub mod namespace;
pub mod new_iteration_reason;
pub mod pkgbase;
pub mod pkgname;
pub mod pkgnames;
pub mod repository_name;
pub mod version;

/// Create the database at the given URL if it doesn't exist,
/// run any migrations that have not run yet, and return a connection to the
/// database.
pub async fn create_migrate_connect(db_file: &Utf8Path) -> Result<DatabaseConnection> {
    let db_url = format!("sqlite://{db_file}?mode=rwc");
    let db = Database::connect(db_url).await?;
    Migrator::up(&db, None).await?;

    Ok(db)
}

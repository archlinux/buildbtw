//! Database migrations for the backend server.
//!
//! Contrary to our style guide, this file is named mod.rs instead of
//! migrations.rs due to the following seaORM issue:
//! <https://github.com/SeaQL/sea-orm/issues/2690>

use sea_orm_migration::prelude::*;

mod m20250811_165601_init;

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20250811_165601_init::Migration)]
    }
}

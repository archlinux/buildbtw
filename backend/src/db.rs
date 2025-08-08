// [sea_orm::DeriveEntityModel] generates qualified references to some types
// so we'll allow this lint in this module to make life easier
#![allow(unused_qualifications)]

use color_eyre::eyre::Result;
use sea_orm::Database;

mod build;

pub async fn create_migrate_connect(db_url: redact::Secret<String>) -> Result<()> {
    let db = Database::connect(db_url.expose_secret()).await?;
    // Check that we can "reach" the sqlite file
    db.ping().await?;

    Ok(())
}

use sea_orm::entity::prelude::*;
use serde::Serialize;
use time::OffsetDateTime;

/// The id of the single row we're storing.
pub const GLOBAL_STATE_ID: &str = "global_state_singleton";

/// Table with a single row for persisting application state that we only need
/// to store once.
/// Inserted and looked up using `GLOBAL_STATE_ID`.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "global_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// Output of the repo updater, used as a cutoff for fetching only repositories
    /// that were updated since the last time the repo updater ran.
    pub source_repos_last_updated: Option<OffsetDateTime>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            id: GLOBAL_STATE_ID.to_string(),
            source_repos_last_updated: None,
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}

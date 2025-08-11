use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};

/// Provides SeaORM compatibility for ALPM package versions.
#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct Version(alpm_types::FullVersion);

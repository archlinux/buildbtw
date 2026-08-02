use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{entities, git};

#[derive(Debug, Serialize, Deserialize)]
pub struct Iteration {
    pub id: Uuid,
    pub created_at: time::OffsetDateTime,

    pub sequence: u32,

    pub status: entities::iterations::Status,

    pub reason: entities::iterations::NewIterationReason,

    pub changesets: git::Changesets,
}

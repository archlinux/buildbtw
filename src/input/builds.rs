use serde::{Deserialize, Serialize};

use crate::package;

#[derive(Debug, Serialize, Deserialize)]
pub struct SetStatus {
    pub status: package::BuildStatus,
}

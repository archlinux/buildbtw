use sea_orm::DeriveValueType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct BranchName(String);

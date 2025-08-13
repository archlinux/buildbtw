use sea_orm::DeriveValueType;
use serde::{Deserialize, Serialize};

/// A git branch name used in package source repositories.
///
/// Provides type safety when working with references to git branches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct BranchName(String);

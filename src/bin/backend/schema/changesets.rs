use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};

use crate::schema::{branch_name::BranchName, repository_name::RepositoryName};

/// A collection of branches in package source repositories.
///
/// Each changeset entry represents a package and its
/// git branch that contains changes to be built.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct Changesets(Vec<(RepositoryName, BranchName)>);

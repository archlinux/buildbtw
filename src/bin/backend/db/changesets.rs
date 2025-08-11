use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};

use crate::db::branch_name::BranchName;

use super::pkgbase::Pkgbase;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct Changesets(Vec<(Pkgbase, BranchName)>);

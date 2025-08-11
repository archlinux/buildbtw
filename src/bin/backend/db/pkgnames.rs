use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};

use super::pkgname::Pkgname;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct Pkgnames(Vec<Pkgname>);

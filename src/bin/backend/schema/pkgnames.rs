use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};

use crate::schema::pkgname::Pkgname;

/// A collection of package names in a PKGBUILD.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct Pkgnames(Vec<Pkgname>);

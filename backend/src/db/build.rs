use sea_orm::{FromJsonQueryResult, entity::prelude::*};
use serde::{Deserialize, Serialize};

use crate::{build_status::BuildStatus, concrete_architecture::ConcreteArchitecture};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "builds")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    architecture: ConcreteArchitecture,
    // TODO: create newtype to distinguish pkgbase from pkgname?
    pkgbase: String,
    branch_name: String,
    commit_hash: String,
    status: BuildStatus,
    version: Version,
    pkgnames: Pkgnames,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
struct Version(alpm_types::FullVersion);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
struct Pkgnames(Vec<String>);

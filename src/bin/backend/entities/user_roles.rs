use sea_orm::entity::prelude::*;
use serde::Serialize;
use strum::{Display, EnumString};

use crate::db_fields::TextUuid;
use crate::entities::users;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Display, EnumString, DeriveValueType, Serialize)]
#[sea_orm(value_type = "String")]
pub enum Role {
    /// Most used role, for dispatching and releasing builds.
    PackageMaintainer,
    /// Can do everything.
    Admin,
}

/// User roles join table.
///
/// Uses a UUID as primary key with a unique constraint on (user_id, role)
/// at the database level to prevent duplicate role assignments.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "user_roles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TextUuid,

    #[sea_orm(unique_key = "user_role")]
    pub user_id: TextUuid,

    #[sea_orm(unique_key = "user_role")]
    pub role: Role,

    #[sea_orm(belongs_to, from = "user_id", to = "id")]
    pub user: HasOne<users::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

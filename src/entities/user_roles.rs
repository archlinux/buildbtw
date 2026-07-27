use crate::api;
use sea_orm::entity::prelude::*;
use serde::Serialize;
use strum::{Display, EnumString};

use crate::db_fields::TxtUuid;
use crate::entities::users;

/// The specific role a user has.
///
/// It gives the user different permissions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Display, EnumString, DeriveValueType, Serialize)]
#[sea_orm(value_type = "String")]
pub enum Role {
    /// Role used by bots.
    Bot,

    /// Most used role, for dispatching and releasing builds.
    PackageMaintainer,

    /// Can do everything.
    Admin,
}

impl From<Role> for api::users::Role {
    fn from(value: Role) -> Self {
        match value {
            Role::Bot => api::users::Role::Bot,
            Role::PackageMaintainer => api::users::Role::PackageMaintainer,
            Role::Admin => api::users::Role::Admin,
        }
    }
}

impl From<api::users::Role> for Role {
    fn from(value: api::users::Role) -> Self {
        match value {
            api::users::Role::Bot => Role::Bot,
            api::users::Role::PackageMaintainer => Role::PackageMaintainer,
            api::users::Role::Admin => Role::Admin,
        }
    }
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
    pub id: TxtUuid,

    #[sea_orm(unique_key = "user_role")]
    pub user_id: TxtUuid,

    #[sea_orm(unique_key = "user_role")]
    pub role: Role,

    #[sea_orm(belongs_to, from = "user_id", to = "id")]
    pub user: BelongsTo<users::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

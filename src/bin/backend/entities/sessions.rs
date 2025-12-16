use sea_orm::entity::prelude::*;
use serde::Serialize;

use crate::db_fields::TextUuid;
use crate::entities::users;

/// Represents an active authenticated session in the application.
///
/// Each record corresponds to a valid session owned by a specific user,
/// allowing them to access protected web endpoints. Sessions are created
/// when a user successfully authenticates and remain valid until they
/// expire or are explicitly removed.
///
/// The session stores its unique identifier, the associated user's Uuid,
/// the creation timestamp, and the last time it was used. This information
/// is used to track user activity and automatically invalidate stale sessions.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    /// Uuid used to reference and identify a specific session
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TextUuid,

    /// Creation time of the session, right after authentication
    pub created_at: time::OffsetDateTime,

    /// The Uuid of the user to whom this session belongs
    pub user_id: TextUuid,

    /// Date-time of the most recent access using this session
    pub last_accessed: time::OffsetDateTime,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "users::Entity",
        from = "Column::UserId",
        to = "users::Column::Id"
    )]
    Iteration,
}

impl Related<users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Iteration.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

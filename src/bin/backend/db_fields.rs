//! Types for usage in SeaORM column definitions.
//! These either make custom types compatible with SeaQuery, or they wrap
//! primitives to prevent mixups in the rest of the codebase.

use derive_more::FromStr;
use redact::Secret;
use sea_orm::{DeriveValueType, TryFromU64};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// The reason why a new build iteration was created.
#[derive(Clone, Debug, PartialEq, Eq, Display, EnumString, DeriveValueType)]
#[sea_orm(value_type = "String")]
pub enum NewIterationReason {
    FirstIteration,
    CreatedByUser,
}

/// Newtype making sure that UUIDs will be stored as SQLite `TEXT` columns,
/// instead of `BLOB` (which is SeaORM's only implementation).
/// - TEXT makes it easier to interact with the SQLite DB directly
/// - Allows for queries like `WHERE id IN (<uuid>, <uuid>, ...)` which are
///   impossible to write with `BLOB` values
///
/// Upstream feature request: <https://github.com/SeaQL/sea-orm/issues/2717>
///
/// Note: Even though the upstream feature request has been merged, we can't
/// make use of it because se need Serialize/Deserialize because we put the
/// SeaORM models into templates (which in turn needs Serialize).
#[derive(Clone, Debug, PartialEq, Eq, Copy, FromStr, Serialize, Deserialize, DeriveValueType)]
#[sea_orm(value_type = "String")]
pub struct TxtUuid(pub uuid::Uuid);

impl std::fmt::Display for TxtUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFromU64 for TxtUuid {
    fn try_from_u64(_n: u64) -> Result<Self, sea_orm::DbErr> {
        Err(sea_orm::DbErr::ConvertFromU64("TxtUuid"))
    }
}

impl From<uuid::Uuid> for TxtUuid {
    fn from(value: uuid::Uuid) -> Self {
        TxtUuid(value)
    }
}

impl From<TxtUuid> for uuid::Uuid {
    fn from(value: TxtUuid) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, FromStr, DeriveValueType)]
#[sea_orm(value_type = "String", to_str = "RedactedString::expose_secret")]
pub struct RedactedString(pub Secret<String>);

impl RedactedString {
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl<T> From<T> for RedactedString
where
    T: AsRef<str>,
{
    fn from(value: T) -> Self {
        RedactedString(Secret::new(value.as_ref().to_owned()))
    }
}

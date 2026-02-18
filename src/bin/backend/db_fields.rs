//! Types for usage in SeaORM column definitions.
//! These either make custom types compatible with SeaQuery, or they wrap
//! primitives to prevent mixups in the rest of the codebase.

use sea_orm::FromJsonQueryResult;
use sea_orm::{
    DeriveValueType, TryFromU64, TryGetable,
    sea_query::{self, ValueType, ValueTypeErr},
};
use serde::{Deserialize, Serialize};

use strum::{Display, EnumString};

/// The reason why a new build iteration was created.
#[derive(Clone, Debug, PartialEq, Eq, Display, EnumString, DeriveValueType)]
#[sea_orm(value_type = "String")]
pub enum NewIterationReason {
    FirstIteration,
    CreatedByUser,
}

/// Provides SeaORM compatibility for ALPM package versions.
#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct Version(alpm_types::FullVersion);

impl From<alpm_types::FullVersion> for Version {
    fn from(value: alpm_types::FullVersion) -> Self {
        Self(value)
    }
}

/// Newtype making sure that UUIDs will be stored as SQLite `TEXT` columns,
/// instead of `BLOB` (which is SeaORM's only implementation).
/// - TEXT makes it easier to interact with the SQLite DB directly
/// - Allows for queries like `WHERE id IN (<uuid>, <uuid>, ...)` which are
///   impossible to write with `BLOB` values
///
/// Upstream feature request: <https://github.com/SeaQL/sea-orm/issues/2717>
#[derive(Clone, Debug, PartialEq, Eq, Copy, Serialize, Deserialize)]
pub struct TextUuid(pub uuid::Uuid);

impl From<TextUuid> for sea_query::Value {
    fn from(value: TextUuid) -> Self {
        sea_query::Value::String(Some(Box::new(value.0.to_string())))
    }
}

impl std::fmt::Display for TextUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryGetable for TextUuid {
    fn try_get_by<I: sea_orm::ColIdx>(
        res: &sea_orm::QueryResult,
        index: I,
    ) -> Result<Self, sea_orm::TryGetError> {
        let uuid_str: String = res.try_get_by(index)?;
        let uuid = uuid::Uuid::parse_str(&uuid_str).map_err(|e| {
            sea_orm::TryGetError::DbErr(sea_orm::DbErr::TryIntoErr {
                from: "String",
                into: "uuid::Uuid",
                source: Box::new(e),
            })
        })?;
        Ok(TextUuid(uuid))
    }
}

impl ValueType for TextUuid {
    fn try_from(v: sea_orm::Value) -> Result<Self, ValueTypeErr> {
        match v {
            sea_orm::Value::String(Some(s)) => {
                let uuid = uuid::Uuid::parse_str(&s).map_err(|_| ValueTypeErr)?;
                Ok(TextUuid(uuid))
            }
            _ => Err(ValueTypeErr),
        }
    }

    fn type_name() -> String {
        "text_uuid".to_string()
    }

    fn array_type() -> sea_query::ArrayType {
        sea_query::ArrayType::String
    }

    fn column_type() -> sea_orm::ColumnType {
        sea_orm::ColumnType::String(sea_query::StringLen::None)
    }
}

impl TryFromU64 for TextUuid {
    fn try_from_u64(_n: u64) -> Result<Self, sea_orm::DbErr> {
        Err(sea_orm::DbErr::ConvertFromU64("TextUuid"))
    }
}

impl AsRef<uuid::Uuid> for TextUuid {
    fn as_ref(&self) -> &uuid::Uuid {
        &self.0
    }
}

impl From<uuid::Uuid> for TextUuid {
    fn from(value: uuid::Uuid) -> Self {
        TextUuid(value)
    }
}

impl From<TextUuid> for uuid::Uuid {
    fn from(value: TextUuid) -> Self {
        value.0
    }
}

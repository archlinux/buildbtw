use sea_orm::DeriveValueType;
use strum::{Display, EnumString};

/// The reason why a new build iteration was created.
#[derive(Clone, Debug, PartialEq, Eq, Display, EnumString, DeriveValueType)]
#[sea_orm(value_type = "String")]
pub enum NewIterationReason {
    FirstIteration,
    CreatedByUser,
}

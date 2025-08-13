use sea_orm::DeriveValueType;
use strum::{Display, EnumString};

/// States a build can be in.
#[derive(Clone, Debug, PartialEq, Eq, Display, EnumString, DeriveValueType)]
#[sea_orm(value_type = "String")]
pub enum BuildStatus {
    /// Other failed builds are blocking this build from running
    Blocked,
    /// This is waiting to be scheduled
    Pending,
    /// Sent to the worker to build
    Scheduled,
    /// Worker has started building
    Building,
    /// Build has succeeded
    Built,
    /// Build as failed
    Failed,
}

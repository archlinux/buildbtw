use sea_orm::DeriveValueType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
/// Newtype to prevent accidental mixups with pkgnames.
pub struct Pkgbase(String);

use sea_orm::DeriveValueType;
use serde::{Deserialize, Serialize};

/// A repository name used for package builds.
///
/// This newtype wrapper provides type safety when working with repository references
/// in the build system. Repository names specify which repository packages should be
/// built for, enabling support for different repositories like core, extra, community, etc.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct RepositoryName(String);

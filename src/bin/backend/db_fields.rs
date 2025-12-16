//! Types for usage in SeaORM column definitions.
//! These either make custom types compatible with SeaQuery, or they wrap
//! primitives to prevent mixups in the rest of the codebase.

use std::sync::LazyLock;

use nutype::nutype;
use regex::Regex;
use sea_orm::{
    DeriveValueType, TryFromU64, TryGetable,
    sea_query::{self, ValueType, ValueTypeErr},
};
use serde::{Deserialize, Serialize};

/// A git branch name used in package source repositories.
///
/// Provides type safety when working with references to git branches.
#[nutype(
    // TODO: add proper validation (https://git-scm.com/docs/git-check-ref-format)
    validate(not_empty),
    derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unsafe(DeriveValueType)
)]
pub struct BranchName(String);

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

impl From<buildbtw::api::builds::Status> for BuildStatus {
    fn from(value: buildbtw::api::builds::Status) -> Self {
        match value {
            buildbtw::api::builds::Status::Blocked => BuildStatus::Blocked,
            buildbtw::api::builds::Status::Pending => BuildStatus::Pending,
            buildbtw::api::builds::Status::Scheduled => BuildStatus::Scheduled,
            buildbtw::api::builds::Status::Building => BuildStatus::Building,
            buildbtw::api::builds::Status::Built => BuildStatus::Built,
            buildbtw::api::builds::Status::Failed => BuildStatus::Failed,
        }
    }
}

use sea_orm::FromJsonQueryResult;

/// A collection of branches in package source repositories.
///
/// Each changeset entry represents a package and its
/// git branch that contains changes to be built.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult, Default)]
pub struct Changesets(Vec<(RepositorySlug, BranchName)>);

/// [`alpm_types::Architecture`], but without the `Any` variant.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, DeriveValueType, EnumString, Display,
)]
#[non_exhaustive]
#[sea_orm(value_type = "String")]
pub enum ConcreteArchitecture {
    /// ARMv8 64-bit
    Aarch64,
    /// ARM
    Arm,
    /// ARMv6 hard-float
    Armv6h,
    /// ARMv7 hard-float
    Armv7h,
    /// Intel 386
    I386,
    /// Intel 486
    I486,
    /// Intel 686
    I686,
    /// Intel Pentium 4
    Pentium4,
    /// RISC-V 32-bit
    Riscv32,
    /// RISC-V 64-bit
    Riscv64,
    /// Intel x86_64
    X86_64,
    /// Intel x86_64 version 2
    #[strum(to_string = "x86_64_v2")]
    X86_64V2,
    /// Intel x86_64 version 3
    #[strum(to_string = "x86_64_v3")]
    X86_64V3,
    /// Intel x86_64 version 4
    #[strum(to_string = "x86_64_v4")]
    X86_64V4,
}

impl AsRef<alpm_types::SystemArchitecture> for ConcreteArchitecture {
    fn as_ref(&self) -> &alpm_types::SystemArchitecture {
        use alpm_types::SystemArchitecture;

        match self {
            ConcreteArchitecture::Aarch64 => &SystemArchitecture::Aarch64,
            ConcreteArchitecture::Arm => &SystemArchitecture::Arm,
            ConcreteArchitecture::Armv6h => &SystemArchitecture::Armv6h,
            ConcreteArchitecture::Armv7h => &SystemArchitecture::Armv7h,
            ConcreteArchitecture::I386 => &SystemArchitecture::I386,
            ConcreteArchitecture::I486 => &SystemArchitecture::I486,
            ConcreteArchitecture::I686 => &SystemArchitecture::I686,
            ConcreteArchitecture::Pentium4 => &SystemArchitecture::Pentium4,
            ConcreteArchitecture::Riscv32 => &SystemArchitecture::Riscv32,
            ConcreteArchitecture::Riscv64 => &SystemArchitecture::Riscv64,
            ConcreteArchitecture::X86_64 => &SystemArchitecture::X86_64,
            ConcreteArchitecture::X86_64V2 => &SystemArchitecture::X86_64V2,
            ConcreteArchitecture::X86_64V3 => &SystemArchitecture::X86_64V3,
            ConcreteArchitecture::X86_64V4 => &SystemArchitecture::X86_64V4,
        }
    }
}

/// The reason why a new build iteration was created.
#[derive(Clone, Debug, PartialEq, Eq, Display, EnumString, DeriveValueType)]
#[sea_orm(value_type = "String")]
pub enum NewIterationReason {
    FirstIteration,
    CreatedByUser,
}

/// Newtype to prevent accidental mixups with pkgnames.
#[nutype(
    validate(predicate = validate_alpm_name),
    derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unsafe(DeriveValueType)
)]
pub struct Pkgbase(String);

impl TryFrom<alpm_types::PackageBaseName> for Pkgbase {
    type Error = PkgbaseError;

    fn try_from(value: alpm_types::PackageBaseName) -> Result<Self, Self::Error> {
        Self::try_new(value.to_string())
    }
}

/// Newtype to prevent accidental mixups with pkgbases.
#[nutype(
    validate(predicate = validate_alpm_name),
    derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unsafe(DeriveValueType)
)]
pub struct Pkgname(String);

fn validate_alpm_name(value: &str) -> bool {
    // Unfortunately, the clone here can't be avoided with the current [alpm_types]
    // API
    alpm_types::Name::new(value).is_ok()
}

/// A collection of package names in a PKGBUILD.
#[nutype(
    validate(predicate = validate_pkgnames),
    derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unsafe(FromJsonQueryResult)
)]
pub struct Pkgnames(Vec<Pkgname>);

fn validate_pkgnames(input: &[Pkgname]) -> bool {
    !input.is_empty()
}

/// A repository name used for package builds.
///
/// This newtype wrapper provides type safety when working with repository
/// references in the build system. Repository names specify which repository
/// packages should be built for, enabling support for different repositories
/// like core, extra, community, etc.
#[nutype(
    // See https://docs.gitlab.com/user/reserved_names/#rules-for-usernames-project-and-group-names-and-slugs
    validate(predicate = validate_repository_name),
    derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unsafe(FromJsonQueryResult)
)]
pub struct RepositorySlug(String);

#[allow(clippy::unwrap_used)]
static REPO_NAME_ALLOWED_CHARS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[a-zA-Z0-9_\\.\\-\\+]+$").unwrap());

#[allow(clippy::unwrap_used)]
static REPO_NAME_OUTER_CHARS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[a-zA-Z0-9].*[a-zA-Z0-9]$").unwrap());

#[allow(clippy::unwrap_used)]
static REPO_SLUG_REPEATED_SPECIAL_CHARS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("[\\-\\+\\_]{2,}").unwrap());

fn validate_repository_name(name: &str) -> bool {
    REPO_NAME_ALLOWED_CHARS.is_match(name)
        && REPO_NAME_OUTER_CHARS.is_match(name)
        && !REPO_SLUG_REPEATED_SPECIAL_CHARS.is_match(name)
        && !name.ends_with(".git")
        && !name.ends_with(".atom")
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

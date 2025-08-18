//! Types for usage in SeaORM column definitions.
//! These either make custom types compatible with SeaQuery, or they wrap
//! primitives to prevent mixups in the rest of the codebase.

use sea_orm::DeriveValueType;
use serde::{Deserialize, Serialize};

/// A git branch name used in package source repositories.
///
/// Provides type safety when working with references to git branches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
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

use sea_orm::FromJsonQueryResult;

/// A collection of branches in package source repositories.
///
/// Each changeset entry represents a package and its
/// git branch that contains changes to be built.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct Changesets(Vec<(RepositoryName, BranchName)>);

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

impl AsRef<alpm_types::Architecture> for ConcreteArchitecture {
    fn as_ref(&self) -> &alpm_types::Architecture {
        use alpm_types::Architecture;

        match self {
            ConcreteArchitecture::Aarch64 => &Architecture::Aarch64,
            ConcreteArchitecture::Arm => &Architecture::Arm,
            ConcreteArchitecture::Armv6h => &Architecture::Armv6h,
            ConcreteArchitecture::Armv7h => &Architecture::Armv7h,
            ConcreteArchitecture::I386 => &Architecture::I386,
            ConcreteArchitecture::I486 => &Architecture::I486,
            ConcreteArchitecture::I686 => &Architecture::I686,
            ConcreteArchitecture::Pentium4 => &Architecture::Pentium4,
            ConcreteArchitecture::Riscv32 => &Architecture::Riscv32,
            ConcreteArchitecture::Riscv64 => &Architecture::Riscv64,
            ConcreteArchitecture::X86_64 => &Architecture::X86_64,
            ConcreteArchitecture::X86_64V2 => &Architecture::X86_64V2,
            ConcreteArchitecture::X86_64V3 => &Architecture::X86_64V3,
            ConcreteArchitecture::X86_64V4 => &Architecture::X86_64V4,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
/// Newtype to prevent accidental mixups with pkgnames.
pub struct Pkgbase(String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
/// Newtype to prevent accidental mixups with pkgbases.
pub struct Pkgname(String);

/// A collection of package names in a PKGBUILD.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct Pkgnames(Vec<Pkgname>);

/// A repository name used for package builds.
///
/// This newtype wrapper provides type safety when working with repository
/// references in the build system. Repository names specify which repository
/// packages should be built for, enabling support for different repositories
/// like core, extra, community, etc.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveValueType)]
pub struct RepositoryName(String);

/// Provides SeaORM compatibility for ALPM package versions.
#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct Version(alpm_types::FullVersion);

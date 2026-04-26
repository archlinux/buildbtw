//! Types for dealing with package-specific data.

use alpm_types::{Architecture, SystemArchitecture};
use camino::Utf8PathBuf;
use color_eyre::Result;
use derive_more::{AsRef, Display, From, FromStr};
use nutype::nutype;
use sea_orm::DeriveValueType;
use serde::{Deserialize, Serialize};

use crate::regex;

/// The name of a concrete package (not a `pkgbase`)
/// This is a newtype because alpm_types only uses type aliases to differentiate between `package_name` and `package_base_name`.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    FromStr,
    From,
    Serialize,
    Deserialize,
    AsRef,
    Display,
    DeriveValueType,
)]
#[sea_orm(value_type = "String", try_from_u64)]
pub struct Name(alpm_types::Name);

/// A collection of package names in a PKGBUILD.
#[nutype(
    validate(predicate = validate_pkgnames),
    derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unchecked(sea_orm::FromJsonQueryResult)
)]
pub struct Names(Vec<Name>);

fn validate_pkgnames(input: &[Name]) -> bool {
    !input.is_empty()
}

/// The base name of a PKGBUILD (not a `pkgname`)
/// This is a newtype because alpm_types only uses type aliases to differentiate between `package_name` and `package_base_name`.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    From,
    FromStr,
    AsRef,
    Display,
    DeriveValueType,
)]
#[sea_orm(value_type = "String", try_from_u64)]
pub struct BaseName(alpm_types::PackageBaseName);

/// A package source repository name.
///
/// This newtype wrapper provides type safety when working with repository
/// references in the build system.
#[nutype(
    // See https://docs.gitlab.com/user/reserved_names/#rules-for-usernames-project-and-group-names-and-slugs
    validate(predicate = validate_repository_name),
    derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, AsRef, Deref, TryFrom, Display),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unchecked(sea_orm::FromJsonQueryResult)
)]
pub struct RepositorySlug(String);

fn validate_repository_name(name: &str) -> bool {
    #![expect(
        clippy::case_sensitive_file_extension_comparisons,
        reason = "Clippy doesn't recognize we've fixed this."
    )]
    let lowercase_name = name.to_ascii_lowercase();
    // set of all allowed chars, no matter the position
    regex!("^[a-zA-Z0-9_\\.\\-\\+]+$").is_match(name)
        // starts with non-special char
        && regex!("^[a-zA-Z0-9].*$").is_match(name)
        // ends with non-special char
        && regex!("^.*[a-zA-Z0-9]$").is_match(name)
        // no consecutive special chars
        && !regex!("[\\-\\+\\_]{2,}").is_match(name)
        && !lowercase_name.ends_with(".git")
        && !lowercase_name.ends_with(".atom")
}

/// [`alpm_types::Architecture`], but with only the architectures buildbtw is interested in building.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    sea_orm::DeriveValueType,
    strum::EnumString,
    strum::EnumIter,
    strum::Display,
)]
#[non_exhaustive]
#[sea_orm(value_type = "String")]
#[strum(serialize_all = "snake_case")]
pub enum KnownArchitecture {
    /// ARMv8 64-bit
    Aarch64,
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

impl AsRef<Architecture> for KnownArchitecture {
    fn as_ref(&self) -> &Architecture {
        match self {
            KnownArchitecture::Aarch64 => &Architecture::Some(SystemArchitecture::Aarch64),
            KnownArchitecture::Riscv32 => &Architecture::Some(SystemArchitecture::Riscv32),
            KnownArchitecture::Riscv64 => &Architecture::Some(SystemArchitecture::Riscv64),
            KnownArchitecture::X86_64 => &Architecture::Some(SystemArchitecture::X86_64),
            KnownArchitecture::X86_64V2 => &Architecture::Some(SystemArchitecture::X86_64V2),
            KnownArchitecture::X86_64V3 => &Architecture::Some(SystemArchitecture::X86_64V3),
            KnownArchitecture::X86_64V4 => &Architecture::Some(SystemArchitecture::X86_64V4),
        }
    }
}

impl From<KnownArchitecture> for Architecture {
    fn from(value: KnownArchitecture) -> Self {
        match value {
            KnownArchitecture::Aarch64 => Architecture::Some(SystemArchitecture::Aarch64),
            KnownArchitecture::Riscv32 => Architecture::Some(SystemArchitecture::Riscv32),
            KnownArchitecture::Riscv64 => Architecture::Some(SystemArchitecture::Riscv64),
            KnownArchitecture::X86_64 => Architecture::Some(SystemArchitecture::X86_64),
            KnownArchitecture::X86_64V2 => Architecture::Some(SystemArchitecture::X86_64V2),
            KnownArchitecture::X86_64V3 => Architecture::Some(SystemArchitecture::X86_64V3),
            KnownArchitecture::X86_64V4 => Architecture::Some(SystemArchitecture::X86_64V4),
        }
    }
}

/// States a build can be in.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    derive_more::Display,
    derive_more::FromStr,
    sea_orm::DeriveValueType,
    Serialize,
    Deserialize,
)]
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

/// Provides SeaORM compatibility for ALPM package versions.
#[nutype(
    derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromStr, From),
    derive_unchecked(sea_orm::FromJsonQueryResult)
)]
pub struct Version(alpm_types::FullVersion);

/// Take a split package for a specific architecture and predict the
/// name of the package file `makepkg` will generate.
/// Additionally takes a [`alpm_srcinfo::SourceInfoV1`] struct to find out if the package
/// is for the `any` architecture.
pub fn file_name(
    alpm_srcinfo::MergedPackage {
        name,
        version,
        architecture,
        ..
    }: &alpm_srcinfo::MergedPackage,
    srcinfo: &alpm_srcinfo::SourceInfoV1,
) -> Result<Utf8PathBuf> {
    // Find the architectures of this split package by checking the split package
    // overrides and taking the base architectures as a fallback.
    let package_architectures = srcinfo
        .packages
        .iter()
        .find(|p| &p.name == name)
        .and_then(|package| package.architectures.as_ref())
        .unwrap_or(&srcinfo.base.architectures);
    // The architecture from MergedPackage reflects the architecture of the whole
    // build graph. But for "any" packages, the filename will instead contain
    // "any", even though the build graph will be for a [`KnownArchictecture`].
    let actual_architecture = if package_architectures == &alpm_types::Architectures::Any {
        &Architecture::Any
    } else {
        architecture
    };
    // Note: Don't use `KnownArchitecture` to determine the architecture in the
    // filename as the filename will contain `any` instead of the known
    // architecture
    Ok(alpm_types::PackageFileName::new(
        name.clone(),
        version.clone(),
        actual_architecture.clone(),
        Some(alpm_types::CompressionAlgorithmFileExtension::Zstd),
    )
    .to_string()
    .into())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::RepositorySlug;

    #[rstest]
    #[case("a_z.A-Z+09a")]
    #[case("z")]
    fn repository_slug_valid(#[case] slug: &str) {
        assert!(
            RepositorySlug::try_new(slug).is_ok(),
            "'{slug}' should be a valid slug"
        );
    }

    #[rstest]
    // May not end with ".git" or ".atom"
    #[case("lemao.git")]
    #[case("lemao.atom")]
    // Needs letter or number at the start and end
    #[case(".sdf-")]
    #[case("+sdf_")]
    #[case("a+")]
    #[case("-z")]
    #[case("afl++")]
    // No consecutive special chars
    #[case("libsigc++-3.0")]
    #[case("a--b")]
    #[case("a__b")]
    #[case("a+_b")]
    fn repository_slug_invalid(#[case] slug: &str) {
        assert!(
            RepositorySlug::try_new(slug).is_err(),
            "'{slug}' should be an invalid slug"
        );
    }
}

//! Types for dealing with package-specific data.

use std::str::FromStr;

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
#[nutype(
    derive(
        Clone,
        Debug,
        PartialEq,
        Eq,
        Hash,
        Serialize,
        Deserialize,
        TryFrom,
        FromStr,
        AsRef,
        Display,
    ),
    derive_unchecked(DeriveValueType),
    validate(with = validate_base_name, error = alpm_types::Error),
)]
#[sea_orm(value_type = "String", try_from_u64)]
pub struct BaseName(alpm_types::PackageBaseName);

impl TryFrom<BaseName> for RepositorySlug {
    type Error = garde::Error;

    /// Convert a package base name to a GitLab-valid repository slug.
    ///
    /// This follows the same transformation as pkgctl's `gitlab_project_name_to_path`:
    /// <https://docs.gitlab.com/ee/user/reserved_names.html>
    ///
    /// 1. Replace single `+` between word boundaries with `-`
    /// 2. Replace any remaining `+` with literal `plus`
    /// 3. Replace any special chars other than `_`, `-` and `.` with `-`
    /// 4. Replace consecutive `_`/`-` chars with a single `-`
    /// 5. Replace exact `tree` with `unix-tree` (GitLab reserved keyword)
    fn try_from(name: BaseName) -> Result<Self, Self::Error> {
        let name = name.to_string();

        // Step 1: Replace '+' between word boundaries with '-'
        // Matches ([a-zA-Z0-9]+)\+([a-zA-Z]+) in the shell script
        let slug = regex!("([a-zA-Z0-9]+)\\+([a-zA-Z]+)")
            .replace_all(&name, "$1-$2")
            .to_string();

        // Step 2: Replace any remaining '+' with 'plus'
        let slug = slug.replace('+', "plus");

        // Step 3: Replace any special chars other than '_', '-' and '.' with '-'
        let slug: String = slug
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                    c
                } else {
                    '-'
                }
            })
            .collect();

        // Step 4: Collapse consecutive '_' or '-' chars with a single '-'
        let slug = regex!("[_\\-]{2,}").replace_all(&slug, "-").to_string();

        // Step 5: Replace exact 'tree' with 'unix-tree'
        let slug = if slug == "tree" {
            "unix-tree".to_string()
        } else {
            slug
        };

        RepositorySlug::try_new(slug)
    }
}

// ALPM does not validate types on deserialization, so until that is fixed,
// we need to do it explicitly
// https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/219
// https://gitlab.archlinux.org/archlinux/alpm/alpm/-/work_items/348
fn validate_base_name(val: &alpm_types::PackageBaseName) -> Result<(), alpm_types::Error> {
    alpm_types::PackageBaseName::from_str(val.as_ref())?;

    Ok(())
}

/// A package source repository name, slugified after gitlab's rules.
///
/// This newtype wrapper provides type safety when working with repository
/// references in the build system.
#[nutype(
    // See https://docs.gitlab.com/user/reserved_names/#rules-for-usernames-project-and-group-names-and-slugs
    validate(with = validate_repository_name, error = garde::Error),
    derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, AsRef, Deref, TryFrom, Display, FromStr),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unchecked(sea_orm::DeriveValueType)
)]
pub struct RepositorySlug(String);

fn validate_repository_name(name: &str) -> Result<(), garde::Error> {
    #![expect(
        clippy::case_sensitive_file_extension_comparisons,
        reason = "Clippy doesn't recognize we've fixed this."
    )]
    let lowercase_name = name.to_ascii_lowercase();
    // set of all allowed chars, no matter the position
    let valid = regex!("^[a-zA-Z0-9_\\.\\-\\+]+$").is_match(name)
        // starts with non-special char
        && regex!("^[a-zA-Z0-9].*$").is_match(name)
        // ends with non-special char
        && regex!("^.*[a-zA-Z0-9]$").is_match(name)
        // no consecutive special chars
        && !regex!("[\\-\\+\\_\\.]{2,}").is_match(name)
        && !lowercase_name.ends_with(".git")
        && !lowercase_name.ends_with(".atom");

    if !valid {
        return Err(garde::Error::new(
            "Must start and end with alphanumeric character, cannot contain consecutive special characters, and cannot end in either '.git' or '.atom'.",
        ));
    }

    Ok(())
}

/// [`alpm_types::Architecture`], but with only the architectures buildbtw is interested in building.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    sea_orm::DeriveValueType,
    strum::EnumString,
    strum::EnumIter,
    strum::Display,
    Serialize,
    Deserialize,
)]
#[non_exhaustive]
#[sea_orm(value_type = "String")]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BuildArchitecture {
    /// ARMv8 64-bit
    // We almost support this, but still need to provision ARM runners
    // and route builds based on architecture
    // Aarch64,
    /// Intel x86_64
    #[default]
    X86_64,
    /// Intel x86_64 version 3
    #[strum(to_string = "x86_64_v3")]
    X86_64V3,
}

impl AsRef<Architecture> for BuildArchitecture {
    fn as_ref(&self) -> &Architecture {
        match self {
            // BuildArchitecture::Aarch64 => &Architecture::Some(SystemArchitecture::Aarch64),
            BuildArchitecture::X86_64 => &Architecture::Some(SystemArchitecture::X86_64),
            BuildArchitecture::X86_64V3 => &Architecture::Some(SystemArchitecture::X86_64V3),
        }
    }
}

impl From<BuildArchitecture> for Architecture {
    fn from(value: BuildArchitecture) -> Self {
        match value {
            // BuildArchitecture::Aarch64 => Architecture::Some(SystemArchitecture::Aarch64),
            BuildArchitecture::X86_64 => Architecture::Some(SystemArchitecture::X86_64),
            BuildArchitecture::X86_64V3 => Architecture::Some(SystemArchitecture::X86_64V3),
        }
    }
}

/// States a build can be in.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    derive_more::Display,
    derive_more::FromStr,
    sea_orm::DeriveValueType,
    Serialize,
    Deserialize,
    strum::EnumIter,
    Hash,
)]
#[sea_orm(value_type = "String")]
pub enum BuildStatus {
    /// Other failed builds are blocking this build from running
    Blocked,

    /// This is waiting to be scheduled
    Pending,

    /// Sent to the worker to build
    /// If `dispatched_to` is `None`, the worker has not yet acknowledged receiving the build yet
    /// If `dispatched_to` is `Some`, the worker has acknowledged the build
    /// and will set the status to `Building` once it starts
    Scheduled,

    /// Worker has started building
    Building,

    /// Build has succeeded
    Built,

    /// Build has failed
    Failed,

    /// Build was skipped, e.g. because its buildspace was stopped.
    Skipped,
}

impl BuildStatus {
    /// Return a color for showing this status on the CLI.
    #[must_use]
    pub fn cli_color(&self) -> yansi::Color {
        match self {
            BuildStatus::Blocked | BuildStatus::Pending | BuildStatus::Scheduled => {
                yansi::Color::Yellow
            }
            BuildStatus::Building => yansi::Color::Blue,
            BuildStatus::Built => yansi::Color::Green,
            BuildStatus::Failed => yansi::Color::Red,
            BuildStatus::Skipped => yansi::Color::White,
        }
    }

    /// Return a unicode symbol representing this status.
    #[must_use]
    pub fn symbol(&self) -> char {
        match self {
            BuildStatus::Blocked => '●',
            // Since this status should only be rarely visible, it's fine to use the same icon as `Scheduled`.
            BuildStatus::Pending | BuildStatus::Scheduled => '⧗',
            BuildStatus::Building => '✦',
            BuildStatus::Built => '✓',
            BuildStatus::Failed => '✗',
            BuildStatus::Skipped => '⌀',
        }
    }
}

/// Provides SeaORM compatibility for ALPM package versions.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    FromStr,
    From,
    AsRef,
    Display,
    sea_orm::DeriveValueType,
)]
#[sea_orm(value_type = "String")]
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
    use std::str::FromStr;

    use rstest::rstest;

    use super::BaseName;
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
    #[case("a..b")]
    #[case("a__b")]
    #[case("a+_b")]
    fn repository_slug_invalid(#[case] slug: &str) {
        assert!(
            RepositorySlug::try_new(slug).is_err(),
            "'{slug}' should be an invalid slug"
        );
    }

    #[rstest]
    #[case("libfoo")]
    #[case("test-package-please++ignore")]
    #[case("libsigc++-3.0")]
    #[case("afl++")]
    #[case("a_z.A-Z+09a")]
    #[case("cowfortune")]
    fn base_name_valid(#[case] pkgbase: &str) {
        assert!(
            BaseName::from_str(pkgbase).is_ok(),
            "'{pkgbase}' should be a valid base name"
        );
    }

    #[rstest]
    #[case("")]
    #[case("-foo")]
    #[case(".foo")]
    #[case("foo bar")]
    #[case("⚡")]
    #[case("lib#bar")]
    #[case("foo$bar")]
    #[case("lemao/noslash")]
    fn base_name_invalid(#[case] pkgbase: &str) {
        assert!(
            BaseName::from_str(pkgbase).is_err(),
            "'{pkgbase}' should be an invalid base name"
        );
    }

    #[test]
    fn base_name_serde_rejects_invalid() {
        assert!(
            serde_json::from_str::<BaseName>("\"-bad\"").is_err(),
            "deserializing an invalid base name should fail"
        );
    }

    #[rstest]
    #[case("libfoo", "libfoo")]
    // '+' between word boundaries becomes '-'
    #[case("foo+bar", "foo-bar")]
    // Consecutive '+' becomes 'plusplus'
    #[case("c++", "cplusplus")]
    #[case("afl++", "aflplusplus")]
    #[case("libsigc++-3.0", "libsigcplusplus-3.0")]
    // '+' followed by digits stays as 'plus'
    #[case("a_z.A-Z+09a", "a_z.A-Zplus09a")]
    // Simple names pass through unchanged
    #[case("cowfortune", "cowfortune")]
    #[case("test-package-please++ignore", "test-package-pleaseplusplusignore")]
    // Exact 'tree' becomes 'unix-tree'
    #[case("tree", "unix-tree")]
    // 'tree' as part of a larger name is unchanged
    #[case("treehouse", "treehouse")]
    fn base_name_to_repository_slug(#[case] input: &str, #[case] expected: &str) {
        let base: BaseName = input.parse().unwrap();
        let slug: RepositorySlug = base.try_into().unwrap();
        assert_eq!(slug.as_ref(), expected);
    }
}

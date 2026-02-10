//! Types for dealing with package-specific data.

use std::str::FromStr;

use crate::regex;
use alpm_types::Architecture;
use alpm_types::SystemArchitecture;
use nutype::nutype;
use sea_orm::sea_query;

/// The name of a concrete package (not a `pkgbase`)
/// This is a newtype because alpm_types only uses type aliases to differentiate between `package_name` and `package_base_name`.
#[nutype(derive(
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
    Deref
))]
pub struct Name(alpm_types::Name);

impl sea_orm::TryGetable for Name {
    fn try_get_by<I: sea_orm::ColIdx>(
        res: &sea_orm::QueryResult,
        index: I,
    ) -> Result<Self, sea_orm::TryGetError> {
        let str: String = res.try_get_by(index)?;
        let parsed = Name::from_str(&str).map_err(|e| {
            sea_orm::TryGetError::DbErr(sea_orm::DbErr::TryIntoErr {
                from: "String",
                into: "package::Name",
                source: Box::new(e),
            })
        })?;
        Ok(parsed)
    }
}

impl From<Name> for sea_query::Value {
    fn from(value: Name) -> Self {
        sea_query::Value::String(Some(Box::new(value.to_string())))
    }
}

impl sea_query::ValueType for Name {
    fn try_from(v: sea_orm::Value) -> Result<Self, sea_query::ValueTypeErr> {
        match v {
            sea_orm::Value::String(Some(s)) => {
                let parsed = Name::from_str(&s).map_err(|_| sea_query::ValueTypeErr)?;
                Ok(parsed)
            }
            _ => Err(sea_query::ValueTypeErr),
        }
    }

    fn type_name() -> String {
        "package_name".to_string()
    }

    fn array_type() -> sea_query::ArrayType {
        sea_query::ArrayType::String
    }

    fn column_type() -> sea_orm::ColumnType {
        sea_orm::ColumnType::String(sea_query::StringLen::None)
    }
}

impl sea_orm::TryFromU64 for Name {
    fn try_from_u64(_n: u64) -> Result<Self, sea_orm::DbErr> {
        Err(sea_orm::DbErr::ConvertFromU64("package::Name"))
    }
}

/// A collection of package names in a PKGBUILD.
#[nutype(
    validate(predicate = validate_pkgnames),
    derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unsafe(sea_orm::FromJsonQueryResult)
)]
pub struct Names(Vec<Name>);

fn validate_pkgnames(input: &[Name]) -> bool {
    !input.is_empty()
}

/// The base name of a PKGBUILD (not a `pkgname`)
/// This is a newtype because alpm_types only uses type aliases to differentiate between `package_name` and `package_base_name`.
#[nutype(derive(
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
    Deref
))]
pub struct BaseName(alpm_types::PackageBaseName);

impl sea_orm::TryGetable for BaseName {
    fn try_get_by<I: sea_orm::ColIdx>(
        res: &sea_orm::QueryResult,
        index: I,
    ) -> Result<Self, sea_orm::TryGetError> {
        let str: String = res.try_get_by(index)?;
        let parsed = BaseName::from_str(&str).map_err(|e| {
            sea_orm::TryGetError::DbErr(sea_orm::DbErr::TryIntoErr {
                from: "String",
                into: "package::BaseName",
                source: Box::new(e),
            })
        })?;
        Ok(parsed)
    }
}

impl From<BaseName> for sea_query::Value {
    fn from(value: BaseName) -> Self {
        sea_query::Value::String(Some(Box::new(value.to_string())))
    }
}

impl sea_query::ValueType for BaseName {
    fn try_from(v: sea_orm::Value) -> Result<Self, sea_query::ValueTypeErr> {
        match v {
            sea_orm::Value::String(Some(s)) => {
                let parsed = BaseName::from_str(&s).map_err(|_| sea_query::ValueTypeErr)?;
                Ok(parsed)
            }
            _ => Err(sea_query::ValueTypeErr),
        }
    }

    fn type_name() -> String {
        "package_base_name".to_string()
    }

    fn array_type() -> sea_query::ArrayType {
        sea_query::ArrayType::String
    }

    fn column_type() -> sea_orm::ColumnType {
        sea_orm::ColumnType::String(sea_query::StringLen::None)
    }
}

impl sea_orm::TryFromU64 for BaseName {
    fn try_from_u64(_n: u64) -> Result<Self, sea_orm::DbErr> {
        Err(sea_orm::DbErr::ConvertFromU64("PackageBaseName"))
    }
}

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
    derive_unsafe(sea_orm::FromJsonQueryResult)
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

/// [`alpm_types::Architecture`], but without the `Any` variant.
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
pub enum KnownArchitecture {
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

impl AsRef<Architecture> for KnownArchitecture {
    fn as_ref(&self) -> &Architecture {
        match self {
            KnownArchitecture::Aarch64 => &Architecture::Some(SystemArchitecture::Aarch64),
            KnownArchitecture::Arm => &Architecture::Some(SystemArchitecture::Arm),
            KnownArchitecture::Armv6h => &Architecture::Some(SystemArchitecture::Armv6h),
            KnownArchitecture::Armv7h => &Architecture::Some(SystemArchitecture::Armv7h),
            KnownArchitecture::I386 => &Architecture::Some(SystemArchitecture::I386),
            KnownArchitecture::I486 => &Architecture::Some(SystemArchitecture::I486),
            KnownArchitecture::I686 => &Architecture::Some(SystemArchitecture::I686),
            KnownArchitecture::Pentium4 => &Architecture::Some(SystemArchitecture::Pentium4),
            KnownArchitecture::Riscv32 => &Architecture::Some(SystemArchitecture::Riscv32),
            KnownArchitecture::Riscv64 => &Architecture::Some(SystemArchitecture::Riscv64),
            KnownArchitecture::X86_64 => &Architecture::Some(SystemArchitecture::X86_64),
            KnownArchitecture::X86_64V2 => &Architecture::Some(SystemArchitecture::X86_64V2),
            KnownArchitecture::X86_64V3 => &Architecture::Some(SystemArchitecture::X86_64V3),
            KnownArchitecture::X86_64V4 => &Architecture::Some(SystemArchitecture::X86_64V4),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RepositorySlug;
    use rstest::rstest;

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

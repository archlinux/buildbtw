use alpm_srcinfo::{MergedPackage, SourceInfoV1, source_info::v1::package::Package};
use alpm_types::Architecture;
use camino::Utf8PathBuf;
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use crate::Pkgname;

pub type SourceInfo = SourceInfoV1;

/// [`alpm_types::Architecture`], but without the `Any` variant.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Display,
    EnumString,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
    sqlx::Type,
    strum::EnumIter,
)]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
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

impl AsRef<Architecture> for ConcreteArchitecture {
    fn as_ref(&self) -> &Architecture {
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

pub fn package_architectures<'a>(
    package: &'a Package,
    source_info: &'a SourceInfo,
) -> &'a Vec<Architecture> {
    match &package.architectures {
        None => &source_info.base.architectures,
        Some(value) => value,
    }
}

/// All architectures used either in the source info base, or in one of its split packages
pub fn source_info_architectures(source_info: &SourceInfo) -> Vec<Architecture> {
    source_info
        .packages
        .iter()
        .fold(source_info.base.architectures.clone(), |mut v, package| {
            if let Some(architectures) = &package.architectures {
                v.extend(architectures);
                v.sort();
                v.dedup();
                v
            } else {
                v.sort();
                v.dedup();
                v
            }
        })
}

pub fn package_for_architecture(
    source_info: &SourceInfo,
    architecture: ConcreteArchitecture,
    pkgname: &Pkgname,
) -> Option<MergedPackage> {
    source_info
        .packages_for_architecture(*architecture.as_ref())
        .find(|p| p.name.as_ref() == pkgname)
}

/// Take a split package for a specific architecture and predict the
/// name of the package file `makepkg` will generate.
/// Additionally takes a [SourceInfo] struct to find out if the package
/// is for the `any` architecture.
pub fn package_file_name(
    MergedPackage {
        name,
        package_version,
        package_release,
        architecture,
        epoch,
        ..
    }: &MergedPackage,
    srcinfo: &SourceInfo,
) -> Result<Utf8PathBuf> {
    // Find the architectures of this split package by checking the split package overrides and taking the base architectures as a fallback.
    let package_architectures = srcinfo
        .packages
        .iter()
        .find(|p| &p.name == name)
        .and_then(|package| package.architectures.as_ref())
        .unwrap_or(&srcinfo.base.architectures);
    // The architecture from MergedPackage reflects the architecture of the whole build graph.
    // But for "any" packages, the filename will instead contain "any", even though the build graph will be for a [`ConcreteArchictecture`].
    let actual_architecture = if package_architectures.contains(&Architecture::Any) {
        &Architecture::Any
    } else {
        architecture
    };
    // TODO: make it work for all compression formats
    // We'll probably have to pass in a directory to search for package files
    // here, similar to `find_cached_package` in devtools
    // (parsing makepkg output seems like an ugly alternative)
    // Note: Don't use `ConcreteArchitecture` to determine the architecture in the filename as the filename will contain `any` instead of the concrete architecture
    let version = alpm_types::Version::new(
        package_version.clone(),
        *epoch,
        Some(package_release.clone()),
    );
    Ok(alpm_types::PackageFileName::new(
        name.clone(),
        version,
        *actual_architecture,
        Some(alpm_types::CompressionAlgorithmFileExtension::Zstd),
    )?
    .to_string()
    .into())
}

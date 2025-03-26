use std::collections::HashSet;

use alpm_srcinfo::{source_info::v1::package::Package, SourceInfoV1};
use alpm_types::Architecture;
use serde::{Deserialize, Serialize};

use crate::BuildPackageOutput;

pub type SourceInfo = SourceInfoV1;

/// [`alpm_types::Architecture`], but without the `Any` variant.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
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
) -> &'a HashSet<Architecture> {
    match &package.architectures {
        None => &source_info.base.architectures,
        Some(value) => value,
    }
}

/// All architectures used either in the source info base, or in one of its split packages
pub fn source_info_architectures(source_info: &SourceInfo) -> HashSet<Architecture> {
    source_info
        .packages
        .iter()
        .fold(source_info.base.architectures.clone(), |set, package| {
            if let Some(architectures) = &package.architectures {
                set.union(architectures).copied().collect()
            } else {
                set
            }
        })
}

pub fn build_outputs(source_info: &SourceInfo) -> Vec<BuildPackageOutput> {
    source_info
        .packages
        .iter()
        .map(|pkg| BuildPackageOutput {
            pkgbase: source_info.base.name.clone().into(),
            pkgname: pkg.name.to_string(),
            arch: package_architectures(pkg, source_info)
                .iter()
                .copied()
                .collect(),
            version: source_info.base.package_version.to_string(),
        })
        .collect()
}

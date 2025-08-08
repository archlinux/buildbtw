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
    strum::Display,
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

use buildbtw::{buildspace, package};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Build identified by build-id or buildspace/pkgbase
    pub build: BuildSource,

    /// Do not keep trying to open the log if not uploaded yet
    pub no_wait: bool,
}

#[derive(Debug, Clone)]
pub struct BuildspacePkgbase {
    /// Name of the buildspace
    pub buildspace: buildspace::Slug,

    /// Pkgbase of the build
    pub pkgbase: package::Name,

    // Architecture of the build to fetch log for
    //
    // Default: x86_64 which is the primary architecture.
    pub architecture: package::BuildArchitecture,

    /// Iteration of the buildspace to fetch log for
    ///
    /// Default: latest iteration
    pub iteration: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum BuildSource {
    /// Build id
    BuildId(Uuid),

    /// Buildspace and pkgbase
    Buildspace(BuildspacePkgbase),
}

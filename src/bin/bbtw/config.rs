use buildbtw::{api_client, buildspace, package};
use color_eyre::Result;
use color_eyre::eyre::{OptionExt, WrapErr, bail};
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

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /// Build identified by build-id or buildspace/pkgbase
    pub build: BuildSource,
}

impl BuildSource {
    pub async fn to_build_id(&self, client: &api_client::ApiClient) -> Result<Uuid> {
        // Fetch build id by buildspace/pkgbase or uuid
        let build_id = match self {
            BuildSource::BuildId(build_id) => *build_id,
            BuildSource::Buildspace(buildspace) => {
                let BuildspacePkgbase {
                    buildspace,
                    iteration,
                    architecture,
                    pkgbase,
                } = buildspace;

                let builds = api_client::builds::list(
                    client,
                    buildspace.clone(),
                    *iteration,
                    Some(*architecture),
                    Some(pkgbase.clone()),
                    None,
                    Some(2),
                )
                .await
                .wrap_err("Failed to find build for buildspace package")?
                .builds;

                if builds.len() > 1 {
                    bail!("Retrieved more builds than expected");
                }

                let build = builds
                    .first()
                    .ok_or_eyre("Failed to find build for buildspace package")?;

                build.id
            }
        };

        Ok(build_id)
    }
}

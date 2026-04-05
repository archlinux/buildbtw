use std::path::PathBuf;

use color_eyre::Result;
use color_eyre::eyre::Context;
use serde::Serialize;

use crate::args::ConfigArgs;

#[derive(Debug, Serialize)]
pub struct BuildConfig {
    builds_dir: PathBuf,
    /// Non-optional directory provided by the gitlab runner. Allows caching stuff between separate runs. Currently unused.
    cache_dir: PathBuf,
}

impl BuildConfig {
    pub fn from_args(args: &ConfigArgs) -> Self {
        let builds_dir = args
            .builds_dir
            .join(format!("{}", args.ci_concurrent_project_id))
            .join(&args.ci_project_path_slug);

        let cache_dir = args
            .cache_dir
            .join(format!("{}", args.ci_concurrent_project_id))
            .join(&args.ci_project_path_slug);

        Self {
            builds_dir,
            cache_dir,
        }
    }
}

/// The Config stage which defines configuration for the build environment in JSON.
///
/// <https://docs.gitlab.com/runner/executors/custom/#config>
pub fn config(args: &ConfigArgs) -> Result<()> {
    let build_config = BuildConfig::from_args(args);
    let json =
        serde_json::to_string_pretty(&build_config).wrap_err("Failed to serialize build config")?;
    println!("{json}");
    Ok(())
}

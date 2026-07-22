use buildbtw::{buildspace, executor::config, external_secrets, package::KnownArchitecture};
use camino::Utf8PathBuf;
use color_eyre::{Result, eyre::Context};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, clap::Parser)]
#[command(name = "buildbtw executor", author, about, version)]
pub struct Args {
    /// Be verbose (e.g. log data of requests or processes).
    /// Provide once to set the log level to "info", twice for "debug" and
    /// thrice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Collect telemetry and allow connecting with `tokio-console`
    #[arg(
        long,
        env = "BUILDBTW_TOKIO_CONSOLE_TELEMETRY",
        default_value = "false"
    )]
    pub tokio_console_telemetry: bool,

    /// GitLab Runner provides the environment variable to define which exit code indicates job failure
    #[arg(long, env = "BUILD_FAILURE_EXIT_CODE", default_value = "1")]
    pub build_failure_exit_code: u8,

    /// SSH connection timeout
    #[arg(long, env = "CUSTOM_ENV_SSH_TIMEOUT", default_value = "120")]
    pub ssh_timeout: u32,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Commands {
    Gitlab(GitlabArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub struct GitlabArgs {
    #[command(subcommand)]
    pub command: Gitlab,
}

/// GitLab Custom executor subcommands
///
/// These can't easily be run locally and are meant to be called by GitLab CI.
///
/// See also: <https://docs.gitlab.com/runner/executors/custom/>
///
/// This is used here: <https://gitlab.archlinux.org/archlinux/infrastructure/-/blob/main/roles/gitlab_runner/templates/config.toml.j2?ref_type=heads#L76>
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, clap::Subcommand)]
pub enum Gitlab {
    /// Config stage that returns a json configuration
    ///
    /// Sometimes you might want to set some settings during execution time.
    /// For example setting a build directory depending on the project ID.
    /// Expects a valid JSON string with specific keys.
    ///
    /// <https://docs.gitlab.com/runner/executors/custom/#config>
    Config(ConfigArgs),

    /// Prepare action is responsible for setting up the environment
    ///
    /// It is creating the virtual machine or container, services or anything else.
    /// After this is done, we expect that the environment is ready to run the job.
    ///
    /// <https://docs.gitlab.com/runner/executors/custom.html#prepare>
    Prepare,

    /// Run stage runs the actual build job
    ///
    /// Unlike the other stages, the run stage is executed multiple times, because
    /// it's split into sub stages listed in `RunStage`.
    ///
    /// <https://docs.gitlab.com/runner/executors/custom.html#run>
    Run(RunArgs),

    /// Cleanup stage to clean up the environments
    ///
    /// This final stage is executed even if one of the previous stages failed.
    /// The main goal for this stage is to clean up any of the environments that
    /// might have been set up. For example, turning off VMs or deleting containers.
    ///
    /// <https://docs.gitlab.com/runner/executors/custom.html#cleanup>
    Cleanup,
}

#[derive(Debug, Clone, clap::Args)]
pub struct ConfigArgs {
    /// Directory that stores build artifacts
    #[arg(
        long,
        env = "BUILDBTW_BUILDS_DIR",
        default_value = "/srv/buildbtw/gitlab/builds"
    )]
    pub builds_dir: Utf8PathBuf,

    /// Directory that stores build caches
    #[arg(
        long,
        env = "BUILDBTW_CACHE_DIR",
        default_value = "/srv/buildbtw/gitlab/cache"
    )]
    pub cache_dir: Utf8PathBuf,

    /// Project ID of the dispatched job
    #[arg(long, env = "CUSTOM_ENV_CI_CONCURRENT_PROJECT_ID")]
    pub ci_concurrent_project_id: u32,

    /// Project path slug of the dispatched job
    #[arg(long, env = "CUSTOM_ENV_CI_PROJECT_PATH_SLUG")]
    pub ci_project_path_slug: String,
}

#[derive(Debug, Clone, clap::Args)]
pub struct RunArgs {
    /// The path to the script that downloads the sources. Created by GitLab Runner for the Custom executor to run
    pub script_path: Utf8PathBuf,

    /// Name of the action of the run stage that should be executed
    #[command(subcommand)]
    pub stage: RunStage,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, clap::Subcommand)]
#[clap(rename_all = "snake_case")]
pub enum RunStage {
    /// Debug information which machine the Job is running on
    PrepareScript,

    /// Prepares the Git configuration, and clone/fetch the repository
    GetSources(GetSourcesArgs),

    /// Extract the cache if any are defined
    RestoreCache,

    /// Download artifacts, if any are defined
    DownloadArtifacts,

    /// Run the actual build
    BuildScript(BuildScriptArgs),

    /// after_script defined from the job
    ///
    /// This script is always called even if any of the previous steps failed
    AfterScript,

    /// Creates an archive of all the cache, if any are defined
    ArchiveCache,

    /// Creates an archive of all the cache, if any are defined
    ///
    /// Only executed when build_script fails
    ArchiveCacheOnFailure,

    /// Upload any artifacts that are defined
    ///
    /// Only executed when build_script was successful
    UploadArtifactsOnSuccess,

    /// Upload any artifacts that are defined
    ///
    /// Only executed when build_script fails
    UploadArtifactsOnFailure,

    /// Deletes all file based variables from disk
    CleanupFileVariables,
}

#[derive(Debug, Clone, clap::Args)]
pub struct GetSourcesArgs {
    /// Directory that stores build artifacts
    #[arg(
        long,
        env = "BUILDBTW_BUILDS_DIR",
        default_value = "/srv/buildbtw/gitlab/builds"
    )]
    pub builds_dir: Utf8PathBuf,
}

impl From<GetSourcesArgs> for config::RunGetSources {
    fn from(GetSourcesArgs { builds_dir }: GetSourcesArgs) -> Self {
        config::RunGetSources { builds_dir }
    }
}

#[derive(Debug, Clone, clap::Args)]
pub struct BuildScriptArgs {
    /// Directory of the project that will be built
    #[arg(long, env = "CUSTOM_ENV_CI_PROJECT_DIR")]
    pub ci_project_dir: Utf8PathBuf,

    /// Buildspace slug
    #[arg(long, env = "CUSTOM_ENV_BUILDSPACE_SLUG", requires_all = ["iteration_seqid", "architecture", "pacman_repository_base_url"])]
    pub buildspace_slug: Option<buildspace::Slug>,

    /// Iteration sequence-id
    #[arg(long, env = "CUSTOM_ENV_ITERATION_SEQID", requires_all = ["buildspace_slug", "architecture", "pacman_repository_base_url"])]
    pub iteration_seqid: Option<u32>,

    /// Build architecture
    #[arg(long, env = "CUSTOM_ENV_ARCHITECTURE", requires_all = ["buildspace_slug", "iteration_seqid", "pacman_repository_base_url"])]
    pub architecture: Option<KnownArchitecture>,

    /// Base URL of the pacman repository that should be injected
    ///
    /// The host should be reachable at 10.0.2.2 since we're using user mode networking.
    /// If no value is provided, no pacman repository will be injected into the build.
    #[arg(long, env = "CUSTOM_ENV_PACMAN_REPOSITORY_BASE_URL", requires_all = ["buildspace_slug", "iteration_seqid", "architecture"])]
    pub pacman_repository_base_url: Option<Url>,

    /// Build uuid
    #[arg(long, env = "CUSTOM_ENV_BUILD_ID", requires_all = ["api_server_url"])]
    pub build_id: Option<Uuid>,

    /// Base URL of the output artifacts collector endpoint that retrieves build results
    ///
    /// If no value is provided, the produced output artifacts will not be uploaded.
    /// If set, requires build ID and API server URL as well.
    /// In development, by default the buildbtw backend is available at <https://buildbtw.localhost:8080/>
    #[arg(long, env = "CUSTOM_ENV_API_SERVER_URL", requires_all = ["build_id", "api_token_path"])]
    pub api_server_url: Option<Url>,

    /// Path to a file containing the API token for authentication
    ///
    /// The token can be passed directly using the `BUILDBTW_EXECUTOR_TOKEN` environment variable.
    /// If set, requires build ID and API server URL as well.
    ///
    /// Precedence:
    ///
    /// 1. `BUILDBTW_EXECUTOR_TOKEN` env var
    /// 2. Contents of file specified by the token path
    /// 3. Contents of $XDG_CONFIG_HOME/buildbtw/BUILDBTW_EXECUTOR_TOKEN
    //
    // `verbatim_doc_comment` preserves newlines in the doc listing above
    #[arg(long, env = "BUILDBTW_EXECUTOR_TOKEN_PATH", verbatim_doc_comment, requires_all = ["api_server_url"])]
    api_token_path: Option<Utf8PathBuf>,
}

impl TryFrom<BuildScriptArgs> for config::RunBuildScript {
    type Error = color_eyre::eyre::Error;

    fn try_from(
        BuildScriptArgs {
            ci_project_dir,
            buildspace_slug,
            iteration_seqid,
            architecture,
            pacman_repository_base_url,
            build_id,
            api_server_url,
            api_token_path: bbtw_token_path,
        }: BuildScriptArgs,
    ) -> Result<Self, Self::Error> {
        let api_token =
            external_secrets::get_optional("BUILDBTW_EXECUTOR_TOKEN", bbtw_token_path.as_deref())?;

        let mut upload_config = None;

        if let Some(api_token) = api_token
            && let Some(api_server_url) = api_server_url
        {
            upload_config = Some(config::Upload {
                api_server_url,
                api_token,
            });
        }

        Ok(config::RunBuildScript {
            ci_project_dir,
            buildspace_slug,
            iteration_seqid,
            architecture,
            pacman_repository_base_url,
            build_id,

            upload_config,
            // When invoked as a standalone binary, always log to the passed file descriptors.
            log_destination: config::LogDestination::InheritStdio,
        })
    }
}

impl From<ConfigArgs> for config::BuildConfig {
    fn from(args: ConfigArgs) -> Self {
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
    let build_config = config::BuildConfig::from(args.clone());
    let json =
        serde_json::to_string_pretty(&build_config).wrap_err("Failed to serialize build config")?;
    println!("{json}");
    Ok(())
}

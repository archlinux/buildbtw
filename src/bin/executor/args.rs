use buildbtw::{buildspace, executor::config, external_secrets, package::BuildArchitecture};
use camino::Utf8PathBuf;
use color_eyre::{
    Result,
    eyre::{Context, OptionExt, bail},
};
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
    /// Check that the executor is ready to operate
    Doctor(DoctorArgs),

    /// GitLab Custom executor subcommands
    ///
    /// These can't easily be run locally and are meant to be called by GitLab CI.
    ///
    /// See also: <https://docs.gitlab.com/runner/executors/custom/>
    ///
    /// This is used here: <https://gitlab.archlinux.org/archlinux/infrastructure/-/blob/main/roles/gitlab_runner/templates/config.toml.j2?ref_type=heads#L76>
    Gitlab(GitlabArgs),
}

#[derive(Debug, Clone, clap::Args, PartialEq)]
pub struct DoctorArgs {
    /// API config for testing authentication
    #[clap(flatten)]
    api_config: Option<ApiConfigArgs>,
}

impl TryFrom<DoctorArgs> for config::DoctorConfig {
    type Error = color_eyre::eyre::Error;

    fn try_from(DoctorArgs { api_config }: DoctorArgs) -> Result<Self, Self::Error> {
        let api_config = match api_config {
            Some(ApiConfigArgs {
                api_server_url,
                api_token_path,
            }) => external_secrets::get_optional(
                "BUILDBTW_EXECUTOR_TOKEN",
                api_token_path.as_deref(),
            )?
            .map(|api_token| config::ApiConfig {
                api_server_url,
                api_token,
            }),
            None => None,
        };

        Ok(config::DoctorConfig { api_config })
    }
}

#[derive(Debug, Clone, clap::Args)]
pub struct GitlabArgs {
    #[command(subcommand)]
    pub command: Gitlab,
}

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

#[derive(Debug, Clone, clap::Args, PartialEq)]
// Two things of note here:
// 1. `requires_all` names members that are not part of this struct. They are actually flattened into
//    here from `api_config`.
// 2. `args` so that clap is even aware that a `build_id` exists and is expected to be provided.
#[group(requires_all = ["api_server_url", "build_id"], args = ["api_server_url", "api_token_path", "build_id"])]
pub struct BuildScriptArgs {
    /// Directory of the project that will be built
    #[arg(long, env = "CUSTOM_ENV_CI_PROJECT_DIR")]
    pub ci_project_dir: Utf8PathBuf,

    /// Architecture to build for
    #[arg(long, env = "CUSTOM_ENV_ARCHITECTURE")]
    pub architecture: BuildArchitecture,

    /// Base URL of the pacman repository that should be injected
    #[clap(flatten)]
    pacman_repository: Option<PacmanRepoArgs>,

    /// API config for uploading build artifacts and updating status
    #[clap(flatten)]
    api_config: Option<ApiConfigArgs>,

    /// Build UUID for API calls
    ///
    /// If set, requires the API server URL as well.
    #[arg(long, env = "CUSTOM_ENV_BUILD_ID")]
    build_id: Option<Uuid>,
}

#[derive(Debug, Clone, clap::Args, PartialEq)]
#[group(requires_all = ["buildspace", "iteration", "architecture", "pacman_repository_base_url"])]
pub struct PacmanRepoArgs {
    /// Buildspace slug
    #[arg(long, env = "CUSTOM_ENV_BUILDSPACE", required = false)]
    pub buildspace: buildspace::Slug,

    /// Iteration sequence-id
    #[arg(long, env = "CUSTOM_ENV_ITERATION", required = false)]
    pub iteration: u32,

    /// Base URL of the pacman repository that should be injected
    ///
    /// For local builds, the host is expected to be reachable at 10.0.2.2 from inside the VM since we're using user mode networking.
    /// For gitlab builds, use the buildbtw server's public base URL.
    /// If no value is provided, no pacman repository will be injected into the build.
    #[arg(long, env = "CUSTOM_ENV_PACMAN_REPOSITORY_BASE_URL", required = false)]
    pub pacman_repository_base_url: Url,
}

#[derive(Debug, Clone, clap::Args, PartialEq)]
pub struct ApiConfigArgs {
    /// Base URL of the output artifacts collector endpoint that retrieves build results
    ///
    /// If no value is provided, the produced output artifacts will not be uploaded.
    //
    // `verbatim_doc_comment` preserves newlines in the doc listing above
    #[arg(
        long,
        env = "CUSTOM_ENV_API_SERVER_URL",
        verbatim_doc_comment,
        required = false
    )]
    pub api_server_url: Url,

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
    #[arg(long, env = "BUILDBTW_EXECUTOR_TOKEN_PATH", verbatim_doc_comment)]
    api_token_path: Option<Utf8PathBuf>,
}

impl TryFrom<BuildScriptArgs> for config::RunBuildScript {
    type Error = color_eyre::eyre::Error;

    fn try_from(
        BuildScriptArgs {
            ci_project_dir,
            pacman_repository,
            api_config,
            build_id,
            architecture,
        }: BuildScriptArgs,
    ) -> Result<Self, Self::Error> {
        let api_config = match (api_config, build_id) {
            (
                Some(ApiConfigArgs {
                    api_server_url,
                    api_token_path,
                }),
                Some(build_id),
            ) => {
                let api_token = external_secrets::get_optional(
                    "BUILDBTW_EXECUTOR_TOKEN",
                    api_token_path.as_deref(),
                )?
                .ok_or_eyre("API endpoint configured but no API token provided")?;
                Some(config::RunBuildScriptApiConfig {
                    api_server_url,
                    api_token,
                    build_id,
                })
            }
            (None, None) => None,
            _ => bail!("API server URL and build ID must be provided together"),
        };

        let pacman_repository = match pacman_repository {
            Some(pacman_repository) => {
                let PacmanRepoArgs {
                    buildspace,
                    iteration,
                    pacman_repository_base_url,
                } = pacman_repository;
                Some(config::PacmanRepo {
                    buildspace,
                    iteration,
                    architecture,
                    pacman_repository_base_url,
                })
            }
            None => None,
        };

        Ok(config::RunBuildScript {
            ci_project_dir,
            pacman_repository,
            api_config,
            // When invoked as a standalone binary, always log to the passed file descriptors.
            log_destination: config::LogDestination::InheritStdio,
            architecture,
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use buildbtw::executor::{self, config};
    use clap::Parser;
    use color_eyre::Result;
    use color_eyre::eyre::bail;
    use rstest::rstest;
    use url::Url;
    use uuid::Uuid;

    use super::*;

    fn build_script_args(args: Args) -> Result<BuildScriptArgs> {
        match args {
            Args {
                command:
                    Commands::Gitlab(GitlabArgs {
                        command:
                            Gitlab::Run(RunArgs {
                                script_path: _,
                                stage: RunStage::BuildScript(build_script),
                            }),
                    }),
                ..
            } => Ok(build_script),
            args => bail!("expected gitlab build_script, got: {args:#?}"),
        }
    }

    fn doctor_args(args: Args) -> Result<DoctorArgs> {
        match args {
            Args {
                command: Commands::Doctor(args),
                ..
            } => Ok(args),
            args => bail!("expected doctor, got: {args:#?}"),
        }
    }

    #[rstest]
    fn test_build_script_args_minimal() -> Result<()> {
        let argv = &[
            "buildbtw-executor",
            "gitlab",
            "run",
            "/tmp/foo",
            "build_script",
            "--ci-project-dir=/tmp/foo",
            "--architecture=x86_64_v3",
        ];
        let env = [
            ("CUSTOM_ENV_CI_PROJECT_DIR", None::<&str>),
            ("CUSTOM_ENV_BUILDSPACE", None::<&str>),
            ("CUSTOM_ENV_ITERATION", None::<&str>),
            ("CUSTOM_ENV_ARCHITECTURE", None::<&str>),
            ("CUSTOM_ENV_PACMAN_REPOSITORY_BASE_URL", None::<&str>),
            ("BUILDBTW_API_SERVER_URL", None::<&str>),
            ("BUILDBTW_EXECUTOR_TOKEN_PATH", None::<&str>),
            ("XDG_CONFIG_HOME", Some("/tmp/doesnotexist")),
        ];

        let args: Args = temp_env::with_vars(env, || Args::parse_from(argv));
        let args = build_script_args(args)?;

        assert_eq!(
            args,
            BuildScriptArgs {
                ci_project_dir: "/tmp/foo".into(),
                architecture: BuildArchitecture::X86_64V3,
                pacman_repository: None,
                api_config: None,
                build_id: None,
            }
        );

        let config: config::RunBuildScript = temp_env::with_vars(env, || args.try_into())?;

        assert_eq!(
            config,
            executor::config::RunBuildScript {
                ci_project_dir: "/tmp/foo".into(),
                architecture: BuildArchitecture::X86_64V3,
                pacman_repository: None,
                api_config: None,
                log_destination: config::LogDestination::InheritStdio,
            }
        );

        Ok(())
    }

    #[rstest]
    fn test_build_script_args_full() -> Result<()> {
        let build_id = Uuid::new_v4();
        let argv = &[
            "buildbtw-executor",
            "gitlab",
            "run",
            "/tmp/foo",
            "build_script",
            "--ci-project-dir=/tmp/foo",
            "--architecture=x86_64",
            "--buildspace=foospace",
            "--iteration=1",
            "--pacman-repository-base-url=https://10.0.2.2",
            "--api-server-url=https://localhost",
            "--build-id",
            &build_id.to_string(),
        ];
        let env = [
            ("CUSTOM_ENV_CI_PROJECT_DIR", None::<&str>),
            ("CUSTOM_ENV_BUILDSPACE", None::<&str>),
            ("CUSTOM_ENV_ITERATION", None::<&str>),
            ("CUSTOM_ENV_ARCHITECTURE", None::<&str>),
            ("CUSTOM_ENV_PACMAN_REPOSITORY_BASE_URL", None::<&str>),
            ("BUILDBTW_API_SERVER_URL", None::<&str>),
            ("BUILDBTW_EXECUTOR_TOKEN_PATH", None::<&str>),
            ("BUILDBTW_EXECUTOR_TOKEN", Some("FOOBAR")),
            ("XDG_CONFIG_HOME", Some("/tmp/doesnotexist")),
        ];

        let args: Args = temp_env::with_vars(env, || Args::parse_from(argv));
        let args = build_script_args(args)?;

        assert_eq!(
            args,
            BuildScriptArgs {
                ci_project_dir: "/tmp/foo".into(),
                architecture: BuildArchitecture::X86_64,
                pacman_repository: Some(PacmanRepoArgs {
                    buildspace: buildspace::Slug::try_from("foospace".to_string())?,
                    iteration: 1u32,
                    pacman_repository_base_url: Url::from_str("https://10.0.2.2")?,
                }),
                api_config: Some(ApiConfigArgs {
                    api_server_url: Url::from_str("https://localhost")?,
                    api_token_path: None,
                }),
                build_id: Some(build_id),
            }
        );

        let config: config::RunBuildScript = temp_env::with_vars(env, || args.try_into())?;

        assert_eq!(
            config,
            executor::config::RunBuildScript {
                ci_project_dir: "/tmp/foo".into(),
                architecture: BuildArchitecture::X86_64,
                pacman_repository: Some(executor::config::PacmanRepo {
                    buildspace: buildspace::Slug::try_from("foospace".to_string())?,
                    iteration: 1u32,
                    architecture: BuildArchitecture::X86_64,
                    pacman_repository_base_url: Url::from_str("https://10.0.2.2")?,
                }),
                api_config: Some(executor::config::RunBuildScriptApiConfig {
                    api_server_url: Url::from_str("https://localhost")?,
                    api_token: redact::Secret::new("FOOBAR".into()),
                    build_id,
                }),
                log_destination: config::LogDestination::InheritStdio,
            }
        );

        Ok(())
    }

    #[rstest]
    fn test_build_script_args_with_api_missing_build_id() {
        let argv = &[
            "buildbtw-executor",
            "gitlab",
            "run",
            "/tmp/foo",
            "build_script",
            "--ci-project-dir=/tmp/foo",
            "--api-server-url=https://localhost",
        ];
        let env = [
            ("CUSTOM_ENV_CI_PROJECT_DIR", None::<&str>),
            ("CUSTOM_ENV_BUILDSPACE", None::<&str>),
            ("CUSTOM_ENV_ITERATION", None::<&str>),
            ("CUSTOM_ENV_ARCHITECTURE", None::<&str>),
            ("CUSTOM_ENV_PACMAN_REPOSITORY_BASE_URL", None::<&str>),
            ("BUILDBTW_API_SERVER_URL", None::<&str>),
            ("BUILDBTW_EXECUTOR_TOKEN_PATH", None::<&str>),
            ("XDG_CONFIG_HOME", Some("/tmp/doesnotexist")),
        ];

        let args = temp_env::with_vars(env, || Args::try_parse_from(argv));
        assert!(args.is_err(), "missing build-id with api-server must fail");
    }

    #[rstest]
    fn test_build_script_args_with_pacman_repo_requires_all() {
        let argv = &[
            "buildbtw-executor",
            "gitlab",
            "run",
            "/tmp/foo",
            "build_script",
            "--ci-project-dir=/tmp/foo",
            "--pacman-repository-base-url=https://10.0.2.2",
        ];
        let env = [
            ("CUSTOM_ENV_CI_PROJECT_DIR", None::<&str>),
            ("CUSTOM_ENV_BUILDSPACE", None::<&str>),
            ("CUSTOM_ENV_ITERATION", None::<&str>),
            ("CUSTOM_ENV_ARCHITECTURE", None::<&str>),
            ("CUSTOM_ENV_PACMAN_REPOSITORY_BASE_URL", None::<&str>),
            ("BUILDBTW_API_SERVER_URL", None::<&str>),
            ("BUILDBTW_EXECUTOR_TOKEN_PATH", None::<&str>),
            ("XDG_CONFIG_HOME", Some("/tmp/doesnotexist")),
        ];

        let args = temp_env::with_vars(env, || Args::try_parse_from(argv));
        assert!(
            args.is_err(),
            "missing option for pacman repository config must fail"
        );
    }

    #[rstest]
    fn test_doctor_args_minimal() -> Result<()> {
        let argv = &["buildbtw-executor", "doctor"];
        let env = [
            ("BUILDBTW_API_SERVER_URL", None::<&str>),
            ("BUILDBTW_EXECUTOR_TOKEN_PATH", None::<&str>),
            ("XDG_CONFIG_HOME", Some("/tmp/doesnotexist")),
        ];

        let args: Args = temp_env::with_vars(env, || Args::parse_from(argv));
        let args = doctor_args(args)?;

        assert_eq!(args, DoctorArgs { api_config: None });

        let config: config::DoctorConfig = args.try_into()?;
        assert_eq!(config, executor::config::DoctorConfig { api_config: None });

        Ok(())
    }

    #[rstest]
    fn test_doctor_args_with_api() -> Result<()> {
        let argv = &[
            "buildbtw-executor",
            "doctor",
            "--api-server-url",
            "https://10.0.2.2",
        ];
        let env = [
            ("BUILDBTW_API_SERVER_URL", None::<&str>),
            ("BUILDBTW_EXECUTOR_TOKEN_PATH", None::<&str>),
            ("BUILDBTW_EXECUTOR_TOKEN", Some("FOOBAR")),
            ("XDG_CONFIG_HOME", Some("/tmp/doesnotexist")),
        ];

        let args: Args = temp_env::with_vars(env, || Args::parse_from(argv));
        let args = doctor_args(args)?;

        assert_eq!(
            args,
            DoctorArgs {
                api_config: Some(ApiConfigArgs {
                    api_server_url: Url::try_from("https://10.0.2.2")?,
                    api_token_path: None,
                })
            }
        );

        let config: config::DoctorConfig = temp_env::with_vars(env, || args.try_into())?;
        assert_eq!(
            config,
            executor::config::DoctorConfig {
                api_config: Some(executor::config::ApiConfig {
                    api_server_url: Url::try_from("https://10.0.2.2")?,
                    api_token: redact::Secret::new("FOOBAR".into()),
                })
            }
        );

        Ok(())
    }

    #[rstest]
    fn test_doctor_args_without_api_token() -> Result<()> {
        let argv = &[
            "buildbtw-executor",
            "doctor",
            "--api-server-url",
            "https://10.0.2.2",
        ];
        let env = [
            ("BUILDBTW_API_SERVER_URL", None::<&str>),
            ("BUILDBTW_EXECUTOR_TOKEN_PATH", None::<&str>),
            ("BUILDBTW_EXECUTOR_TOKEN", None::<&str>),
            ("XDG_CONFIG_HOME", Some("/tmp/doesnotexist")),
        ];

        let args: Args = temp_env::with_vars(env, || Args::parse_from(argv));
        let args = doctor_args(args)?;

        assert_eq!(
            args,
            DoctorArgs {
                api_config: Some(ApiConfigArgs {
                    api_server_url: Url::try_from("https://10.0.2.2")?,
                    api_token_path: None,
                })
            }
        );

        let config: config::DoctorConfig = temp_env::with_vars(env, || args.try_into())?;
        assert_eq!(config, executor::config::DoctorConfig { api_config: None });

        Ok(())
    }
}

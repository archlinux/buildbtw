//! Executor for buildbtw that bridges CLI invocation and GitLab instrumentation to the build
//! infrastructure to produce build artifacts.
//!
//! <https://docs.gitlab.com/runner/executors/custom/>

use std::process::ExitCode;

use args::{Args, Commands, Gitlab, RunArgs, RunStage};
use buildbtw::{
    executor::{cleanup, prepare, run},
    graceful_shutdown::shutdown_signal,
};
use clap::Parser;
use color_eyre::Result;
use tokio_util::sync::CancellationToken;
use tracing::info;

mod args;

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();

    if let Err(error) = execute(args.clone()).await {
        eprintln!("{error:?}");
        // Always fail with GitLab runner provided exit code
        // <https://docs.gitlab.com/runner/executors/custom/#build-failure>
        return ExitCode::from(args.build_failure_exit_code);
    }
    ExitCode::SUCCESS
}

/// Executes actions supported by this executor as provided via `Args`.
///
/// The execution dispatcher also takes care of setting up the execution environment
/// like telemetry, error report handler etc.
async fn execute(args: Args) -> Result<()> {
    buildbtw::error_handler::init(args.verbose)?;
    buildbtw::tracing::init(args.verbose, args.tokio_console_telemetry)?;

    match args.command {
        Commands::Gitlab(ref gitlab_args) => match &gitlab_args.command {
            Gitlab::Config(config_args) => args::config(config_args)?,
            Gitlab::Prepare => prepare::prepare(args.ssh_timeout).await?,
            Gitlab::Run(run_args) => run(args.ssh_timeout, run_args).await?,
            Gitlab::Cleanup => cleanup::cleanup().await?,
        },
    }

    Ok(())
}

/// Runs a specific action from the run stage.
///
/// The run stage is executed multiple times, because it’s split into sub stages.
/// STDOUT and STDERR returned from this executable prints to the job log.
///
/// <https://docs.gitlab.com/runner/executors/custom/#run>
pub async fn run(ssh_timeout: u32, run_args: &RunArgs) -> Result<()> {
    let cancellation_token = CancellationToken::new();
    tokio::spawn(shutdown_signal(cancellation_token.clone()));
    match run_args.stage.clone() {
        RunStage::GetSources(get_sources_args) => {
            run::get_sources(&run_args.script_path, get_sources_args.into()).await?;
        }
        RunStage::BuildScript(build_script_args) => {
            run::build_script(
                ssh_timeout,
                build_script_args.try_into()?,
                cancellation_token,
            )
            .await?;
        }
        _ => info!("Unhandled run stage: {:?}", run_args.stage),
    }
    Ok(())
}

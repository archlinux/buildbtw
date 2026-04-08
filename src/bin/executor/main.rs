//! Executor for buildbtw that bridges CLI invocation and GitLab instrumentation to the build
//! infrastructure to produce build artifacts.
//!
//! <https://docs.gitlab.com/runner/executors/custom/>

mod args;
mod cleanup;
mod config;
mod prepare;
mod run;
mod shell;

#[cfg(test)]
mod tests;

use clap::Parser;
use color_eyre::Result;

use std::process::ExitCode;

use crate::args::{Args, Command};

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

    match args.command.clone() {
        Command::Config(config_args) => config::config(&config_args)?,
        Command::Prepare => prepare::prepare(args).await?,
        Command::Run(run_args) => run::run(args, run_args).await?,
        Command::Cleanup => cleanup::cleanup().await?,
    }

    Ok(())
}

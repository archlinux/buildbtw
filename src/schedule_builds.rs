use color_eyre::{
    Result,
    eyre::{OptionExt, eyre},
};
use derive_more::Display;
use sea_orm::DatabaseTransaction;

use crate::{gitlab_api, queries};

#[derive(Debug)]
pub enum Config {
    Gitlab(gitlab_api::Config),
    Local,
}

#[derive(Display, Debug, Clone, PartialEq, Eq, Copy)]
pub enum DispatchBuildsTo {
    /// Create a gitlab pipeline for each build.
    GitlabPipelines,
    /// Run builds by spawning vmexec processes from the server.
    LocalExecutor,
}

impl Config {
    pub fn new(
        dispatch_builds_to: Option<DispatchBuildsTo>,
        maybe_gitlab: Option<gitlab_api::Config>,
    ) -> Result<Option<Config>> {
        Ok(match dispatch_builds_to {
            Some(DispatchBuildsTo::GitlabPipelines) => {
                Some(Config::Gitlab(maybe_gitlab.ok_or_eyre(
                    "Gitlab config must be set for dispatching builds to gitlab pipelines",
                )?))
            }
            Some(DispatchBuildsTo::LocalExecutor) => Some(Config::Local),
            None => None,
        })
    }
}

/// Find all builds that are ready to build and either create gitlab pipelines for them or mark them to be built locally.
pub async fn schedule_pending_builds(config: &Config, tx: &DatabaseTransaction) -> Result<()> {
    let pending = queries::builds::pending(None).all(tx).await?;

    for build in &pending {
        match config {
            Config::Local => {
                queries::builds::dispatch_to_local_executor(build.id)
                    .exec(tx)
                    .await?
            }
            Config::Gitlab(_) => {
                return Err(eyre!(
                    "Dispatching builds to gitlab is not implemented yet."
                ));
            }
        };
    }

    Ok(())
}

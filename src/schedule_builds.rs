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
        match dispatch_builds_to {
            Some(DispatchBuildsTo::GitlabPipelines) => {
                let config = maybe_gitlab.ok_or_eyre(
                    "Gitlab config must be set for dispatching builds to gitlab pipelines",
                )?;
                Ok(Some(Config::Gitlab(config)))
            }
            Some(DispatchBuildsTo::LocalExecutor) => Ok(Some(Config::Local)),
            None => Ok(None),
        }
    }
}

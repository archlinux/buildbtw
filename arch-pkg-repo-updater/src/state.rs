use std::path::PathBuf;
use std::{fs, io};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct State {
    #[serde(with = "time::serde::iso8601::option")]
    pub last_updated: Option<OffsetDateTime>,
}

#[derive(Error, Debug)]
pub enum StateDirError {
    #[error("xdg directories error")]
    XdgDirectoriesError,
}

#[derive(Error, Debug)]
pub enum LoadStateError {
    #[error("io error: {0:#}")]
    IoError(#[from] io::Error),
    #[error("toml error: {0:#}")]
    TomlError(#[from] toml::de::Error),
    #[error("{0:#}")]
    StateDirError(#[from] StateDirError),
}

#[derive(Error, Debug)]
pub enum SaveStateError {
    #[error("io error: {0:#}")]
    IoError(#[from] io::Error),
    #[error("toml error: {0:#}")]
    TomlError(#[from] toml::ser::Error),
    #[error("{0:#}")]
    StateDirError(#[from] StateDirError),
}

impl State {
    pub fn from_filesystem() -> Result<Self, LoadStateError> {
        // acquire config location
        let state_file = Self::state_file()?;

        // return default config if it doesn't exist
        if !state_file.as_path().exists() {
            return Ok(State::default());
        }

        // load config into struct
        let content = fs::read_to_string(state_file)?;
        let config: State = toml::from_str(&content)?;

        Ok(config)
    }

    /// Write the configuration struct to the filesystem as toml
    pub fn write_to_filesystem(&self) -> Result<(), SaveStateError> {
        let state_dir = Self::state_dir()?;
        fs::create_dir_all(&state_dir)?;
        let state_file = Self::state_file()?;
        let content = toml::to_string(&self)?;
        fs::write(state_file, content)?;
        Ok(())
    }

    pub fn state_dir() -> Result<PathBuf, StateDirError> {
        let project_dir = ProjectDirs::from("org", "archlinux", "arch-pkg-repo-updater")
            .ok_or(StateDirError::XdgDirectoriesError)?;
        project_dir
            .state_dir()
            .map(|path| path.into())
            .ok_or(StateDirError::XdgDirectoriesError)
    }

    pub fn state_file() -> Result<PathBuf, StateDirError> {
        Ok(Self::state_dir()?.join("state.toml"))
    }
}

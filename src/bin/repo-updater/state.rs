//! Data to persist between runs of the repo-updater.
use camino::Utf8Path;
use color_eyre::{Result, eyre::ContextCompat};
use std::{collections::HashMap, fs, path::PathBuf};

use buildbtw::xdg_dirs;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct State {
    /// For each directory (stored as an absolute path), remember the datetime of the newest activity we've seen in a git repo inside.
    last_updated: HashMap<PathBuf, OffsetDateTime>,
}

impl State {
    pub fn from_filesystem() -> Result<Self> {
        // acquire config location
        let state_file = Self::xdg_state_file()?;

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
    pub fn write_to_filesystem(&self) -> Result<()> {
        let state_dir = Self::xdg_state_dir()?;
        fs::create_dir_all(&state_dir)?;
        let state_file = Self::xdg_state_file()?;
        let content = toml::to_string(&self)?;
        fs::write(state_file, content)?;
        Ok(())
    }

    /// For the given directory, retrieve the date it was last updated at.
    pub fn target_dir_last_updated(&self, path: &Utf8Path) -> Result<Option<&OffsetDateTime>> {
        let absolute_path = path.canonicalize()?;
        Ok(self.last_updated.get(&absolute_path))
    }

    /// For the given directory, set the date it was last updated at.
    pub fn set_target_dir_last_updated(
        &mut self,
        path: &Utf8Path,
        updated: OffsetDateTime,
    ) -> Result<()> {
        let absolute_path = path.canonicalize()?;
        self.last_updated.insert(absolute_path, updated);
        Ok(())
    }

    fn xdg_state_dir() -> Result<PathBuf> {
        let project_dir = xdg_dirs::new()?;
        Ok(project_dir
            .state_dir()
            .wrap_err("Missing XDG state dir")?
            .to_path_buf())
    }

    fn xdg_state_file() -> Result<PathBuf> {
        Ok(Self::xdg_state_dir()?.join("repo_updater_state.toml"))
    }
}

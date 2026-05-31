// TODO: find a better name for this module
// The name should express:
// - it runs as a background task
// - it updates source repos
// - it creates iterations when build graphs change due to new commits
// - it calculates build graphs for iterations that don't have one yet

//! Background tokio task for calculating build graphs
//!
//! - regularly fetches source repos, looks for new commits, creates new iterations if necessary
//! - calculates build graphs for newly created buildspaces
//!
//! This is centralized in a single task to prevent race conditions around source
//! repo updates. This way, build graphs are calculated on top of a consistent set of
//! source repo commits.
//! As a secondary benefit, this allows easy sharing of a single [`dependency_graph::SourceRepoCache`] struct.
//!
//! Flow:
//!
//! 1. Update all source repos, then populate the SourceRepoCache.
//! 2. Create a list of all known, "old" buildspaces
//! 3. Process all iterations with pending build graphs to make sure
//!    their builds are dispatched as fast as possible, since users are probably waiting for them.
//! 4. Recalculate the build graph for an old buildspace and create a new iteration if it changed
//! 5. Repeat step 3 and 4 until the list of old buildspaces is empty.
//! 6. Start from step 1 with a fresh SourceRepoCache.
//!
//! For debugging, it's possible to disable the source repo updates,
//! and the automatic creation of new iterations.
//! The calculation of pending build graphs cannot be disabled currently.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::{
    dependency_graph::{self, BuildGraphs},
    gitlab::GitlabConfig,
    package::KnownArchitecture,
    repo_updater,
};
use camino::Utf8PathBuf;
use color_eyre::eyre::{OptionExt, Result};
use gitlab::AsyncGitlab;
use sea_orm::{DatabaseConnection, TransactionTrait};
use tokio_util::sync::CancellationToken;

use crate::{entities, queries};

/// State of the task.
#[derive(Debug)]
pub struct IterationCreator {
    config: Config,
    db: DatabaseConnection,
}

#[derive(Debug)]
pub struct Config {
    pub source_repo_dir: Utf8PathBuf,
    pub repo_update: RepoUpdateConfig,

    /// Whether to create new iterations when build graphs change due to new commits.
    /// When this is disabled, build graphs will still be calculated for new buildspaces or manually created iterations.
    pub auto_create_iterations: bool,
}

#[derive(Debug)]
pub enum RepoUpdateConfig {
    DontUpdate,
    DoUpdate(GitlabConfig),
}

impl IterationCreator {
    /// Create a new [`IterationCreator`] but don't run it.
    #[must_use]
    pub fn new(config: Config, db: DatabaseConnection) -> Self {
        Self { config, db }
    }

    /// Spawn a new IterationCreator task.
    pub fn spawn(config: Config, db: DatabaseConnection, token: CancellationToken) {
        let creator = IterationCreator::new(config, db);

        tokio::spawn(creator.run(token));
    }

    /// Continuously run the whole process in a loop.
    /// Check the module description for an overview.
    pub async fn run(mut self, token: CancellationToken) {
        while !token.is_cancelled() {
            let run_start = Instant::now();
            let sleep_duration = if let Err(e) = self.tick().await {
                let retry_duration = Duration::from_secs(30);
                tracing::error!(
                    ?e,
                    "Failed to run iteration creator, retrying in {retry_duration:?}"
                );
                Duration::from_secs(30)
            } else if run_start.elapsed() < Duration::from_secs(10) {
                // If the task took a very short time to complete, wait a bit to make sure we're
                // not spamming it
                Duration::from_secs(10)
            } else {
                Duration::ZERO
            };

            tokio::select! {
                () = token.cancelled() => {},
                () = tokio::time::sleep(sleep_duration) => {}
            }
        }
    }

    /// Tick once: update all source repos, create pending build graphs and new iterations.
    /// TODO: check if it's safe to cancel this function, because it can hold up the shutdown process.
    #[tracing::instrument(skip(self))]
    pub async fn tick(&mut self) -> Result<()> {
        if let RepoUpdateConfig::DoUpdate(gitlab_config) = &self.config.repo_update {
            let gitlab_client = gitlab::GitlabBuilder::new(
                gitlab_config
                    .domain
                    .host_str()
                    .ok_or_eyre("GitLab domain URL has no host")?,
                gitlab_config.token.clone().expose_secret(),
            )
            .build_async()
            .await?;

            self.update_repos(&gitlab_client, gitlab_config.clone())
                .await?;
        }

        let mut source_repo_cache =
            dependency_graph::SourceRepoCache::new(&self.config.source_repo_dir).await?;

        if self.config.auto_create_iterations
            && let Err(e) = self.create_new_iterations(&mut source_repo_cache).await
        {
            tracing::error!(?e, "Failed to create new iterations");
        }

        // Always calculate pending build graphs, even when create_new_iterations is not called.
        if let Err(e) = self
            .calculate_pending_build_graphs(&mut source_repo_cache)
            .await
        {
            tracing::error!(?e, "Could not calculate missing build graphs");
        }

        Ok(())
    }

    /// Calculate build graphs and create iterations
    ///
    /// 1. For iterations pending calculation
    /// 2. For buildspaces where a newly calculated build graph differs from the stored one
    #[tracing::instrument(skip(self, source_repo_cache))]
    async fn create_new_iterations(
        &mut self,
        source_repo_cache: &mut dependency_graph::SourceRepoCache,
    ) -> Result<()> {
        let existing_buildspaces = queries::buildspaces::list().all(&self.db).await?;

        for buildspace in existing_buildspaces {
            // First, calculate build graphs for new iterations that don't have one yet.
            // We do this here to make sure their builds are dispatched as fast as possible,
            // since users are probably waiting for them.
            if let Err(e) = self.calculate_pending_build_graphs(source_repo_cache).await {
                tracing::error!(?e, "Could not calculate missing build graphs");
            }

            // Process one buildspace
            if let Err(e) = self
                .check_buildspace_graph(source_repo_cache, buildspace)
                .await
            {
                tracing::error!(?e, "Failed to check buildspace for build graph changes");
            }
        }

        Ok(())
    }

    /// Check if this buildspace needs a new iteration because of build graph changes.
    #[tracing::instrument(skip(self, source_repos))]
    async fn check_buildspace_graph(
        &self,
        source_repos: &mut dependency_graph::SourceRepoCache,
        buildspace: entities::buildspaces::ModelEx,
    ) -> Result<()> {
        // Find the iteration with the newest timestamp.
        let Some(newest_iteration) = queries::iterations::newest_for_buildspace(buildspace.id)
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };
        let new_graph = BuildGraphs::calculate(&newest_iteration.changesets, source_repos).await?;

        // Fetch all builds of the old iteration and group them by architecture
        let old_builds = queries::builds::by_iteration_id(newest_iteration.id)
            .all(&self.db)
            .await?;

        let mut old_builds_by_architecture: HashMap<
            KnownArchitecture,
            Vec<dependency_graph::BuildNode>,
        > = HashMap::new();

        for build in old_builds {
            old_builds_by_architecture
                .entry(build.architecture)
                .or_default()
                .push(build.into());
        }

        // Diff and create new iteration if needed
        let diffs = new_graph.diff(old_builds_by_architecture);
        if diffs.iter().all(|(_, diff)| diff.is_empty()) {
            // Nothing changed
            return Ok(());
        }

        tracing::info!(
            ?buildspace.name,
            "Creating new iteration due to changed build graph"
        );

        queries::iterations::insert(
            buildspace.id.into(),
            newest_iteration.sequence + 1,
            newest_iteration.changesets,
            entities::iterations::NewIterationReason::BuildGraphChanged,
        )
        .exec(&self.db)
        .await?;

        Ok(())
    }

    /// Find iterations missing build graphs, calculate the graphs, and store them.
    #[tracing::instrument(skip(self, source_repos))]
    async fn calculate_pending_build_graphs(
        &self,
        source_repos: &mut dependency_graph::SourceRepoCache,
    ) -> Result<()> {
        let iterations = queries::iterations::pending_calculation()
            .all(&self.db)
            .await?;

        for iteration in iterations {
            // Calculate graphs for all architectures
            let graphs = BuildGraphs::calculate(&iteration.changesets, source_repos).await?;

            // Insert the graphs
            let tx = self.db.begin().await?;
            for (arch, graph) in graphs.into_inner() {
                let (update_iteration, insert_builds, insert_dependencies) =
                    queries::builds::insert_builds_with_dependencies(
                        iteration.id.into(),
                        arch,
                        &graph,
                    )?;

                update_iteration.exec(&tx).await?;
                insert_builds.exec(&tx).await?;
                insert_dependencies.exec(&tx).await?;
            }

            tx.commit().await?;
        }

        Ok(())
    }

    /// Fetch new commits for all source repositories.
    #[tracing::instrument(skip(self, gitlab_client, gitlab_config))]
    async fn update_repos(
        &self,
        gitlab_client: &AsyncGitlab,
        gitlab_config: GitlabConfig,
    ) -> Result<()> {
        // Get the previous update cutoff timestamp
        let source_repos_last_updated = self
            .db
            .transaction(|tx| Box::pin(queries::global_state::get().one(tx)))
            .await?
            .and_then(|model| model.source_repos_last_updated);

        // Update the source repos
        let source_repos_new_last_updated = repo_updater::update_all_source_repos(
            self.config.source_repo_dir.clone(),
            gitlab_client,
            source_repos_last_updated,
            gitlab_config,
        )
        .await?;

        // Persist the new update cutoff timestamp
        if let Some(new_updated) = source_repos_new_last_updated {
            // Use a new transaction here instead of holding the one above to prevent
            // a long-running transaction that might block other database work.
            self.db
                .transaction(|tx| Box::pin(queries::global_state::upsert(new_updated).exec(tx)))
                .await?;
        }

        Ok(())
    }
}

//! This module runs a background task that performs periodic maintenance jobs
//! for the server. It is started once when the application begins and runs
//! alongside the Axum web server until the cancellation token is triggered
//! for a graceful shutdown.
//!
//! The worker wakes up in preconfigured intervals and executes small housekeeping jobs.
//!
//! The main entry point is [`initialize`] to spawn the background task.

use std::collections::HashSet;

use color_eyre::Result;
use sea_orm::{
    ColumnTrait, DatabaseConnection, DatabaseTransaction, PaginatorTrait, QueryFilter,
    TransactionTrait,
};
use time::Duration;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, trace, warn};

use crate::entities::user_roles;
use crate::gitlab_api;
use crate::{db_fields::TxtUuid, queries, server_state::ServerState};
use crate::{iteration_creator, schedule_builds, storage};

/// Starts background tasks.
///
/// Launches asynchronous tasks that run in the background while the server is running.
/// It does not block the main application thread.
/// Will cancel its tasks when the given cancellation token is cancelled.
///
/// Tasks:
///
/// - Iteration creator for updating source repos, creating iterations and calculating build graphs
/// - Regularly sync OIDC roles from OIDC provider
/// - Regularly delete expired sessions
/// - Dispatch builds to local executor or gitlab pipelines
pub fn initialize(
    state: ServerState,
    token: CancellationToken,
    gitlab_config: Option<gitlab_api::Config>,
    update_source_repos: bool,
    auto_create_iterations: bool,
    dispatch_builds: Option<schedule_builds::Config>,
    db: DatabaseConnection,
) -> Result<()> {
    // If the flag is enabled, and a gitlab config is present, tell the iteration creator to update source repos
    let repo_update_config = if update_source_repos && let Some(gitlab_config) = gitlab_config {
        iteration_creator::RepoUpdateConfig::DoUpdate(gitlab_config)
    } else {
        iteration_creator::RepoUpdateConfig::DontUpdate
    };

    iteration_creator::IterationCreator::spawn(
        iteration_creator::Config {
            repo_update: repo_update_config,
            source_repo_dir: storage::package_source_repos_dir(&state.data_dir)?,
            auto_create_iterations,
        },
        db,
        token.clone(),
    );

    if let Some(dispatch_config) = dispatch_builds {
        spawn_schedule_builds(state.clone(), token.clone(), dispatch_config);
    }

    spawn_invalidate_old_sessions(state.clone(), token.clone());

    // Run OIDC role sync if OIDC is configured
    if let Some(oidc_state) = state.oidc {
        spawn_sync_oidc_roles(state.db, oidc_state, token);
    }

    Ok(())
}

fn spawn_schedule_builds(
    state: ServerState,
    token: CancellationToken,
    dispatch_config: schedule_builds::Config,
) {
    tokio::spawn(async move {
        let mut every_10_seconds = interval(std::time::Duration::from_secs(10));
        every_10_seconds.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = every_10_seconds.tick() => {
                    if let Err(e) = schedule_all_builds(&state, &dispatch_config).await {
                        error!(?e, "Failed to dispatch builds");
                    }
                }
                // Stop gracefully when the provided [`CancellationToken`] is cancelled
                () = token.cancelled() => {
                    break;
                }
            }
        }
    });
}

async fn schedule_all_builds(state: &ServerState, config: &schedule_builds::Config) -> Result<()> {
    let tx = state.db.begin().await?;

    schedule_builds::schedule_pending_builds(config, &tx).await?;

    tx.commit().await?;

    Ok(())
}

fn spawn_invalidate_old_sessions(state: ServerState, token: CancellationToken) {
    tokio::spawn(async move {
        let mut every_hour = interval(std::time::Duration::from_hours(1));
        loop {
            tokio::select! {
                _ = every_hour.tick() => {
                    if let Err(e) = invalidate_old_sessions(&state).await {
                        error!(?e, "Failed to invalidate old sessions");
                    }
                }
                // Stop gracefully when the provided [`CancellationToken`] is cancelled
                () = token.cancelled() => {
                    break;
                }
            }
        }
    });
}

fn spawn_sync_oidc_roles(
    db: DatabaseConnection,
    oidc_config: crate::oidc::State,
    token: CancellationToken,
) {
    tokio::spawn(async move {
        // Make sure this interval is lower than the default refresh token lifespan,
        // since expired refresh tokens will lead to users being logged out from all clients
        let mut every_ten_minutes = interval(std::time::Duration::from_mins(10));
        loop {
            tokio::select! {
                _ = every_ten_minutes.tick() => {
                    if let Err(e) = sync_user_roles_from_oidc(&db, &oidc_config).await {
                        error!(?e, "Failed to sync user roles from OIDC");
                    }
                }
                // Stop gracefully when the provided [`CancellationToken`] is cancelled
                () = token.cancelled() => {
                    break;
                }
            }
        }
    });
}

/// Removes inactive user sessions from the database.
///
/// Deletes all sessions that have not been accessed for more than
/// four weeks, helping to keep the sessions table clean and invalidate
/// old lingering sessions.
#[instrument(skip_all)]
pub async fn invalidate_old_sessions(state: &ServerState) -> Result<()> {
    debug!("Invalidating old sessions");
    let tx = state.db.begin().await?;

    // Delete the old sessions
    let affected_sessions = queries::sessions::delete_old_sessions(Duration::weeks(4))
        .exec_with_returning(&tx)
        .await?;
    info!("Invalidated {} old sessions", affected_sessions.len());

    // For each affected user, check if they have any sessions left
    for session in affected_sessions {
        if let Err(e) = clear_refresh_token_if_no_sessions(&tx, session.user_id.into()).await {
            warn!(?e, user_id = %session.user_id, "Failed to clear refresh token");
            // Continue with other users even if this fails
        }
    }

    tx.commit().await?;

    Ok(())
}

/// Clear a user's refresh token if they have no remaining sessions.
/// This is a security measure to ensure logged-out users don't have
/// valid refresh tokens lingering in the database.
#[instrument(skip_all)]
pub async fn clear_refresh_token_if_no_sessions(
    tx: &DatabaseTransaction,
    user_id: uuid::Uuid,
) -> Result<()> {
    use crate::queries;

    // Check if user has any remaining sessions
    let session_count = queries::sessions::by_user_id(user_id.into())
        .count(tx)
        .await?;

    // If no sessions remain, clear the refresh token
    if session_count == 0 {
        debug!(user_id = %user_id, "User has no sessions, clearing refresh token");
        queries::users::clear_refresh_token(tx, user_id).await?;
    }

    Ok(())
}

/// Syncs all user roles from OIDC provider based on current group memberships.
///
/// For each user in the database, fetches their current groups from the OIDC
/// provider using their stored refresh token, and updates their roles accordingly.
/// Continues on individual user failures to ensure all users are processed.
#[allow(clippy::too_many_lines)]
#[instrument(skip_all)]
pub async fn sync_user_roles_from_oidc(
    db: &DatabaseConnection,
    oidc_config: &crate::oidc::State,
) -> Result<()> {
    use sea_orm::EntityTrait;

    use crate::entities::users;
    use crate::queries;

    debug!("Syncing user roles from OIDC provider");
    let tx = db.begin().await?;

    // Fetch all users from database
    let users = users::Entity::find().all(&tx).await?;

    let total_users = users.len();
    let mut synced_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;

    for user in users {
        // Skip users without refresh tokens (e.g., created before migration)
        let Some(refresh_token) = user.refresh_token else {
            debug!(
                user_id = %user.id.0,
                "Skipping user without refresh token"
            );
            skipped_count += 1;
            continue;
        };

        // Fetch current user info from OIDC
        let (user_info, new_refresh_token) =
            match crate::oidc::fetch_user_info_with_refresh_token(oidc_config, refresh_token).await
            {
                Ok(info) => info,
                Err(e) => {
                    error_count += 1;
                    warn!(
                        ?e,
                        user_id = %user.id.0,
                        oidc_id = %user.oidc_id,
                        "Failed to fetch user info from OIDC, revoking all sessions"
                    );

                    if let Err(e) = revoke_user_sessions_and_roles(&tx, user.id).await {
                        error!(?e);
                    }
                    continue;
                }
            };

        // Update refresh token if a new one was provided (refresh token rotation)
        if let Some(new_token) = new_refresh_token
            && let Err(e) =
                queries::users::update_refresh_token(&tx, user.id.0, Some(new_token)).await
        {
            warn!(
                ?e,
                user_id = %user.id.0,
                "Failed to update refresh token in database"
            );
            // Continue with role sync even if refresh token update fails
        }

        // Map OIDC groups to roles
        let new_roles = crate::oidc::oidc_groups_to_user_roles(
            user_info.additional_claims(),
            &oidc_config.admin_oidc_groups,
            &oidc_config.package_maintainer_oidc_groups,
        );

        // Fetch current roles from database
        let current_roles: Vec<user_roles::Role> = user_roles::Entity::find()
            .filter(user_roles::COLUMN.user_id.eq(user.id.0))
            .all(&tx)
            .await?
            .into_iter()
            .map(|model| model.role)
            .collect();

        // Only update if roles have changed
        let current_roles_set: HashSet<_> = current_roles.iter().collect();
        let new_roles_set: HashSet<_> = new_roles.iter().collect();
        let roles_changed = current_roles_set != new_roles_set;

        if roles_changed {
            // Update user roles in database
            if let Err(e) = queries::user_roles::set(&tx, user.id, new_roles.clone()).await {
                error!(
                    ?e,
                    user_id = %user.id.0,
                    "Failed to update user roles in database"
                );
                error_count += 1;
                continue;
            }

            debug!(
                user_id = %user.id.0,
                ?current_roles,
                ?new_roles,
                "Updated user roles"
            );
            synced_count += 1;
        } else {
            trace!(
                user_id = %user.id.0,
                "User roles unchanged, skipping database update"
            );
            skipped_count += 1;
        }
    }

    tx.commit().await?;

    info!(
        total_users,
        synced_count,
        skipped_count, // Users without refresh tokens + users with unchanged roles
        error_count,
        "Completed OIDC role sync"
    );

    Ok(())
}

async fn revoke_user_sessions_and_roles(tx: &DatabaseTransaction, user_id: TxtUuid) -> Result<()> {
    queries::user_roles::set(tx, user_id, Vec::new()).await?;

    queries::sessions::delete_by_user_id(user_id)
        .exec(tx)
        .await?;

    Ok(())
}

use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, ExprTrait, Insert, QueryFilter, Select};
use uuid::Uuid;

use crate::{
    entities::{builds, gitlab_pipelines},
    gitlab_api, package,
};

#[must_use]
pub fn insert(
    build: &builds::WithIterationAndBuildspace,
    create_response: &gitlab_api::pipelines::CreatePipelineResponse,
) -> Insert<gitlab_pipelines::ActiveModel> {
    let model = gitlab_pipelines::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        build_id: Set(build.id),
        project_id: Set(create_response.project_id),
        pipeline_id: Set(create_response.id),
        web_url: Set(create_response.web_url.to_string()),
    };

    gitlab_pipelines::Entity::insert(model)
}

/// Select all gitlab pipelines whose associated build is dispatched to GitLab
/// and still in an unfinished status (`Scheduled` or `Building`).
///
/// These are pipelines that should be polled for status updates.
#[must_use]
pub fn running() -> Select<gitlab_pipelines::Entity> {
    gitlab_pipelines::Entity::find()
        .inner_join(builds::Entity)
        .filter(
            builds::COLUMN
                .dispatched_to
                .eq(builds::DispatchedTo::Gitlab)
                .and(builds::COLUMN.status.is_in([
                    package::BuildStatus::Scheduled,
                    package::BuildStatus::Building,
                ])),
        )
}

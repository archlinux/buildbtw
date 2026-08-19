use sea_orm::{ActiveValue::Set, EntityTrait, Insert};
use uuid::Uuid;

use crate::{
    entities::{self, gitlab_pipelines},
    gitlab_api,
};

#[must_use]
pub fn insert(
    build: &entities::builds::WithIterationAndBuildspace,
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

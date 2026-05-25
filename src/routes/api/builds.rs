use axum::{Json, body::Bytes, extract::Query};
use color_eyre::eyre::{OptionExt, eyre};
use sea_orm::{PaginatorTrait, SelectExt};

use crate::{
    api::builds::{self, ListBuildsResponse},
    response_error::ResponseError,
};
use crate::{db, queries, response_error::ResponseResult};

pub async fn list(
    _: builds::ListByStatus,
    Query(builds::ListByStatusQuery {
        status,
        buildspace_name,
        max_results,
    }): Query<builds::ListByStatusQuery>,
    db::Tx(tx): db::Tx,
) -> ResponseResult<Json<ListBuildsResponse>> {
    if let Some(buildspace_name) = &buildspace_name {
        let buildspace_exists = queries::buildspaces::by_name(buildspace_name)
            .exists(&tx)
            .await?;

        if !buildspace_exists {
            return Err(ResponseError::NotFound("buildspace".to_string()));
        }
    }

    let builds = queries::builds::list(status, buildspace_name.as_deref(), max_results)
        .all(&tx)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    let total_build_count = queries::builds::list(status, buildspace_name.as_deref(), None)
        .count(&tx)
        .await?;

    Ok(Json(ListBuildsResponse {
        total_build_count,
        builds,
    }))
}

pub async fn upload_package(
    _: builds::UploadPackage,
    Query(builds::UploadPackageQuery { build_id, pkgname }): Query<builds::UploadPackageQuery>,
    db::Tx(tx): db::Tx,
    body: Bytes,
) -> ResponseResult<()> {
    let build = queries::builds::by_id(build_id.into())
        .one(&tx)
        .await?
        .ok_or_eyre("Build job not found")?;

    let filenames = build.pkgnames_filenames.0;

    let pkgbase = build.pkgbase;
    let filename = filenames
        .get(&pkgname)
        .ok_or_else(|| eyre!("Build job has no pkgname '{}'", pkgname))?;

    tracing::info!(
        "Received {} bytes for build-id {} pkgbase {} pkgname {} filename {}",
        body.len(),
        build_id,
        pkgbase,
        pkgname,
        filename,
    );
    Ok(())
}

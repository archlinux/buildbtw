use axum::{Json, extract::Query};
use buildbtw::api::builds;

use crate::{db, db_fields, queries, response_error::ResponseResult};

pub async fn list_by_status(
    _: builds::ListByStatus,
    Query(builds::ListByStatusQuery { status }): Query<builds::ListByStatusQuery>,
    db::Tx(tx): db::Tx,
) -> ResponseResult<Json<Vec<builds::Build>>> {
    let builds = queries::builds::list(status.map(db_fields::BuildStatus::from))
        .all(&tx)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(Json(builds))
}

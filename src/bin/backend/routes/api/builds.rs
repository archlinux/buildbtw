use axum::{Json, extract::Query};
use buildbtw::api::builds;

use crate::{db, queries, response_error::ResponseResult};

pub async fn list(
    _: builds::ListByStatus,
    Query(builds::ListByStatusQuery { status }): Query<builds::ListByStatusQuery>,
    db::Tx(tx): db::Tx,
) -> ResponseResult<Json<Vec<builds::Build>>> {
    let builds = queries::builds::list(status)
        .all(&tx)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(Json(builds))
}

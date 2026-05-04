use crate::from_request;
use crate::templates;

use crate::web;
use axum::response::{Html, IntoResponse};

pub async fn index(
    _: web::index::Index,
    session: Option<from_request::AuthUser>,
) -> crate::response_error::ResponseResult<impl IntoResponse> {
    Ok(Html(templates::index::render_index_page(
        session.as_ref().map(|session| &session.user),
    )?))
}

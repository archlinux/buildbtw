use axum::response::Redirect;
use axum_extra::extract::PrivateCookieJar;
use buildbtw::web;

use crate::{
    db,
    from_request::{self},
    queries,
    response_error::ResponseResult,
};

/// See [web::account::Logout].
pub async fn logout(
    _: web::account::Logout,
    session: from_request::AuthUser,
    cookie_jar: PrivateCookieJar,
    db::Tx(tx): db::Tx,
) -> ResponseResult<(PrivateCookieJar, Redirect)> {
    let _ = queries::sessions::delete(session.session.id)
        .exec(&tx)
        .await?;
    tx.commit().await?;

    let cookie_jar = from_request::sessions::remove_from_cookie_jar(cookie_jar);
    Ok((cookie_jar, Redirect::to(&web::builds::Index {}.to_string())))
}

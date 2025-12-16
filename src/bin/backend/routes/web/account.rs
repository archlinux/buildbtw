use axum::response::{Html, Redirect};
use axum_extra::extract::PrivateCookieJar;
use buildbtw::web;
use color_eyre::eyre::Context;
use sea_orm::{ColumnTrait, QueryFilter};
use uuid::Uuid;

use crate::{
    db,
    db_fields::TextUuid,
    entities::sessions,
    from_request::{self},
    queries,
    response_error::ResponseResult,
    templates,
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
    Ok((cookie_jar, Redirect::to(&web::index::Index {}.to_string())))
}

/// See [web::account::SessionList].
pub async fn session_list(
    _: web::account::SessionList,
    session: from_request::AuthUser,
    cookie_jar: PrivateCookieJar,
    db::Tx(tx): db::Tx,
) -> ResponseResult<(PrivateCookieJar, Html<String>)> {
    let sessions: Vec<sessions::Model> = queries::sessions::by_user_id(session.user.id)
        .all(&tx)
        .await?;

    Ok((
        cookie_jar,
        Html(templates::account::render_session_list_page(
            &session.user,
            sessions,
        )?),
    ))
}

/// See [web::account::SessionRevoke].
pub async fn session_revoke(
    params: web::account::SessionRevoke,
    session: from_request::AuthUser,
    cookie_jar: PrivateCookieJar,
    db::Tx(tx): db::Tx,
) -> ResponseResult<(PrivateCookieJar, Redirect)> {
    tracing::debug!("Session to revoke: {:?}", params.session_id);
    let session_to_revoke: Uuid = params
        .session_id
        .parse()
        .wrap_err("Could not parse UUID from cookie")?;

    let _ = queries::sessions::delete(TextUuid::from(session_to_revoke))
        .filter(sessions::Column::UserId.eq(session.user.id))
        .exec(&tx)
        .await?;
    tx.commit().await?;

    // TODO: remove this once https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/196 is implemented
    let redirect = if session.session.id.0.to_string().eq(&params.session_id) {
        &web::index::Index {}.to_string()
    } else {
        &web::account::SessionList {}.to_string()
    };

    Ok((cookie_jar, Redirect::to(redirect)))
}

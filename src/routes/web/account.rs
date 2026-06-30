use crate::web;
use axum::response::{Html, Redirect};
use axum_extra::extract::PrivateCookieJar;
use color_eyre::eyre::{Context, eyre};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::{
    db,
    db_fields::TxtUuid,
    entities::sessions,
    from_request::{self},
    permissions::{can_revoke_session, permission_ok},
    queries,
    response_error::ResponseResult,
    templates,
};

/// See [web::account::Overview].
pub async fn overview(
    _: web::account::Overview,
    session: from_request::AuthUser,
    cookie_jar: PrivateCookieJar,
) -> ResponseResult<(PrivateCookieJar, Html<String>)> {
    Ok((
        cookie_jar,
        Html(templates::account::render_account_overview(&session.user)?),
    ))
}

/// See [web::account::Logout].
pub async fn logout(
    _: web::account::Logout,
    session: from_request::AuthUser,
    cookie_jar: PrivateCookieJar,
    db::Tx(tx): db::Tx,
) -> ResponseResult<(PrivateCookieJar, Redirect)> {
    let user_id = session.user.id.0;

    let _ = queries::sessions::delete(session.session.id)
        .exec(&tx)
        .await?;

    // Clear refresh token if user has no more sessions
    if let Err(e) = crate::tasks::clear_refresh_token_if_no_sessions(&tx, user_id).await {
        warn!(?e, user_id = %user_id, "Failed to clear refresh token on logout");
    }

    tx.commit().await?;

    let cookie_jar = from_request::auth_user::remove_from_cookie_jar(cookie_jar);
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
            &sessions,
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
    debug!("Session to revoke: {:?}", params.session_id);
    let session_to_revoke: Uuid = params
        .session_id
        .parse()
        .wrap_err("Could not parse UUID from cookie")?;

    permission_ok(can_revoke_session(&tx, &session, session_to_revoke).await)?;

    // Get the user_id before deleting the session
    let session_model = queries::sessions::by_id(session_to_revoke)
        .one(&tx)
        .await?
        .ok_or_else(|| eyre!("Session not found"))?;
    let user_id = session_model.user_id.0;

    let _ = queries::sessions::delete(TxtUuid::from(session_to_revoke))
        .exec(&tx)
        .await?;

    // Clear refresh token if user has no more sessions
    if let Err(e) = crate::tasks::clear_refresh_token_if_no_sessions(&tx, user_id).await {
        warn!(?e, user_id = %user_id, "Failed to clear refresh token on session revoke");
    }

    tx.commit().await?;

    // TODO: remove this once https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/196 is implemented
    let redirect = if session.session.id.0.to_string().eq(&params.session_id) {
        &web::index::Index {}.to_string()
    } else {
        &web::account::SessionList {}.to_string()
    };

    Ok((cookie_jar, Redirect::to(redirect)))
}

/// See [web::account::CliSessionLanding].
pub async fn cli_session_landing(
    _: web::account::CliSessionLanding,
    session: from_request::AuthUser,
    cookie_jar: PrivateCookieJar,
) -> ResponseResult<(PrivateCookieJar, Html<String>)> {
    Ok((
        cookie_jar,
        Html(templates::account::render_create_cli_session_page(
            &session.user,
            None,
        )?),
    ))
}

/// See [web::account::CliSessionCreate].
pub async fn cli_session_create(
    _: web::account::CliSessionCreate,
    session: from_request::AuthUser,
    cookie_jar: PrivateCookieJar,
    db::Tx(tx): db::Tx,
) -> ResponseResult<(PrivateCookieJar, Html<String>)> {
    let session_model = queries::sessions::insert(session.user.id.0, sessions::ClientType::Cli)
        .exec_with_returning(&tx)
        .await?;

    tx.commit().await?;

    let secret_token = session_model.secret_token.expose_secret().to_string();

    Ok((
        cookie_jar,
        Html(templates::account::render_create_cli_session_page(
            &session.user,
            Some(&secret_token),
        )?),
    ))
}

use axum::{
    extract::{Query, State},
    response::Redirect,
};
use axum_extra::extract::PrivateCookieJar;
use buildbtw::web;

use crate::{
    db,
    oidc::{self},
    queries,
    response_error::ResponseResult,
    server_state::ServerState,
};

/// See [web::oidc::StartLogin].
pub async fn start_login(
    _: web::oidc::StartLogin,
    State(server_state): State<ServerState>,
    cookie_jar: PrivateCookieJar,
) -> ResponseResult<(PrivateCookieJar, Redirect)> {
    let (url, login_attempt) = oidc::start_login(server_state.oidc.get_config()?).await?;
    let cookie_jar = login_attempt.save_in_cookie_jar(cookie_jar)?;
    Ok((cookie_jar, Redirect::to(url.as_str())))
}

/// See [web::oidc::Authorized].
pub async fn authorized(
    _: web::oidc::Authorized,
    Query(oidc_query): Query<web::oidc::LoginRedirectQuery>,
    State(server_state): State<ServerState>,
    cookie_jar: PrivateCookieJar,
    db::Tx(tx): db::Tx,
) -> ResponseResult<()> {
    let oidc_config = server_state.oidc.get_config()?;
    let login_attempt = oidc::LoginAttempt::from_cookie_jar(cookie_jar)?;
    let user_info = oidc::convert_authorization_code_to_user_info(
        oidc_config,
        login_attempt,
        oidc_query.code,
        oidc_query.state,
    )
    .await?;
    tracing::debug!(?user_info, "User authorized via OIDC");

    queries::users::upsert(user_info.subject().to_string())
        .exec(&tx)
        .await?;

    Ok(())
}

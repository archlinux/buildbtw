use std::ops::Deref;

use axum::{
    extract::{Query, State},
    response::Redirect,
};
use axum_extra::extract::PrivateCookieJar;
use buildbtw::web;
use color_eyre::eyre::ContextCompat;
use openidconnect::LocalizedClaim;

use crate::{
    db,
    entities::sessions::ClientType,
    from_request, input,
    oidc::{self, oidc_groups_to_user_roles},
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
    let oidc_config = server_state.oidc.get_config()?;
    let (url, login_attempt) = oidc::new_login_attempt(oidc_config.clone());
    let cookie_jar = login_attempt.save_in_cookie_jar(cookie_jar, &oidc_config, &url)?;
    Ok((cookie_jar, Redirect::to(url.as_str())))
}

/// See [web::oidc::Authorized].
pub async fn authorized(
    _: web::oidc::Authorized,
    Query(oidc_query): Query<web::oidc::LoginRedirectQuery>,
    State(server_state): State<ServerState>,
    cookie_jar: PrivateCookieJar,
    db::Tx(tx): db::Tx,
) -> ResponseResult<(PrivateCookieJar, Redirect)> {
    let oidc_config = server_state.oidc.get_config()?;
    let admin_oidc_groups = oidc_config.admin_oidc_groups.clone();
    let package_maintainer_oidc_groups = oidc_config.package_maintainer_oidc_groups.clone();
    let login_attempt = oidc::LoginAttempt::from_cookie_jar(&cookie_jar)?;
    let (user_info, refresh_token) = oidc::convert_authorization_code_to_user_info(
        oidc_config.clone(),
        login_attempt,
        oidc_query.code,
        oidc_query.state,
    )
    .await?;
    tracing::debug!(?user_info, "User authorized via OIDC");

    let username = claim_to_string(user_info.nickname())
        .or(user_info.preferred_username().map(|n| n.to_string()))
        .or(claim_to_string(user_info.name()))
        .wrap_err("Neither nickname nor name provided")?;

    let create = input::users::ValidatedCreate::try_new(input::users::Create {
        oidc_id: user_info.subject().to_string(),
        username,
    })?;

    // Performs an upsert operation to ensure user consistency.
    //
    // This creates a new user record on first login or updates the existing
    // user with the latest data owned by the SSO provider, keeping user
    // information in sync across logins.
    let user = queries::users::upsert(create, refresh_token)
        .exec_with_returning(&tx)
        .await?;

    let session = queries::sessions::insert(user.id.into(), ClientType::Web)
        .exec_with_returning(&tx)
        .await?;

    let roles = oidc_groups_to_user_roles(
        user_info.additional_claims(),
        &admin_oidc_groups,
        &package_maintainer_oidc_groups,
    );

    queries::user_roles::set(&tx, user.id, roles).await?;

    tx.commit().await?;

    let cookie_jar = from_request::auth_user::save_in_cookie_jar(&session.secret_token, cookie_jar);

    Ok((cookie_jar, Redirect::to(&web::index::Index {}.to_string())))
}

/// Convert an optional [LocalizedClaim] into an optional string by taking the
/// value for the first language available.
fn claim_to_string<T: Deref<Target = String>>(claim: Option<&LocalizedClaim<T>>) -> Option<String> {
    claim
        .and_then(|claim| claim.iter().next())
        .map(|(_, nickname)| nickname.to_string())
}

use axum::{Form, response::Html};
use axum_extra::extract::PrivateCookieJar;
use color_eyre::eyre::Context;
use uuid::Uuid;

use crate::{
    db,
    entities::sessions,
    from_request, input,
    permissions::{self},
    queries,
    response_error::{ResponseError, ResponseResult},
    templates, web,
};

/// See [web::admin::BotList].
pub async fn bot_list(
    _: web::admin::BotList,
    session: from_request::AuthUser,
    cookie_jar: PrivateCookieJar,
    db::Tx(tx): db::Tx,
) -> ResponseResult<(PrivateCookieJar, Html<String>)> {
    permissions::check(permissions::can_manage_bots(&session))?;

    let bots = queries::users::bots().all(&tx).await?;

    Ok((
        cookie_jar,
        Html(templates::admin::render_bot_list_page(
            &session.user,
            &bots,
        )?),
    ))
}

/// See [web::admin::BotCreate].
pub async fn bot_create(
    _: web::admin::BotCreate,
    session: from_request::AuthUser,
    cookie_jar: PrivateCookieJar,
    db::TxImmediate(tx): db::TxImmediate,
    Form(body): Form<input::users::CreateBot>,
) -> ResponseResult<(PrivateCookieJar, Html<String>)> {
    permissions::check(permissions::can_manage_bots(&session))?;

    let validated: input::users::CreateWithRoles =
        input::users::ValidatedCreateWithRoles::try_from(input::users::CreateWithRoles::from(
            body,
        ))?
        .into();

    // Create the new bot user...
    let bot = queries::users::insert(validated.username)
        .exec_with_returning(&tx)
        .await?;
    queries::user_roles::set(
        &tx,
        bot.id,
        validated.user_roles.into_iter().map(Into::into).collect(),
    )
    .await?;
    // ...along with its new bot session.
    let bot_session = queries::sessions::insert(bot.id.0, sessions::ClientType::Bot)
        .exec_with_returning(&tx)
        .await?;

    tx.commit().await?;

    Ok((
        cookie_jar,
        Html(templates::admin::render_bot_token_page(
            &session.user,
            &bot,
            bot_session.secret_token.expose_secret(),
        )?),
    ))
}

/// See [web::admin::BotTokenRegenerate].
pub async fn bot_token_regenerate(
    params: web::admin::BotTokenRegenerate,
    session: from_request::AuthUser,
    cookie_jar: PrivateCookieJar,
    db::TxImmediate(tx): db::TxImmediate,
) -> ResponseResult<(PrivateCookieJar, Html<String>)> {
    permissions::check(permissions::can_manage_bots(&session))?;

    let bot_id: Uuid = params
        .user_id
        .parse()
        .wrap_err("Could not parse UUID from path")?;

    let bot = queries::users::bot_by_id(bot_id)
        .one(&tx)
        .await?
        .ok_or_else(|| ResponseError::NotFound("Bot not found".into()))?;

    // A bot only has one session/token at a time so here we'll first delete all sessions by this bot...
    queries::sessions::delete_by_user_id(bot.id)
        .exec(&tx)
        .await?;

    // ...and then insert its new sole session.
    let bot_session = queries::sessions::insert(bot.id.0, sessions::ClientType::Bot)
        .exec_with_returning(&tx)
        .await?;

    tx.commit().await?;

    Ok((
        cookie_jar,
        Html(templates::admin::render_bot_token_page(
            &session.user,
            &bot,
            bot_session.secret_token.expose_secret(),
        )?),
    ))
}

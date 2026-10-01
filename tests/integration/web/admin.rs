use buildbtw::{
    api,
    entities::{sessions::ClientType, user_roles},
    input::users::CreateBot,
    queries, web,
};
use color_eyre::Result;
use rstest::rstest;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, SelectExt};
use uuid::Uuid;

use crate::factories;
use crate::test_ctx::{TestCtx, ctx};

/// Admins see the bot list with existing bots
#[rstest]
#[tokio::test]
async fn test_bot_list(#[future(awt)] ctx: TestCtx) -> Result<()> {
    factories::bot(&ctx.state.db, "some-bot").await?;

    let response = ctx
        .server
        .typed_get(&web::admin::BotList {})
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    response.assert_status_ok();
    response.assert_header("content-type", "text/html; charset=utf-8");
    response.assert_text_contains("some-bot");

    Ok(())
}

/// The account overview links to the bot page for admins only
#[rstest]
#[case::admin(vec![user_roles::Role::Admin], true)]
#[case::package_maintainer(vec![user_roles::Role::PackageMaintainer], false)]
#[tokio::test]
async fn test_account_overview_bot_link(
    #[future(awt)] ctx: TestCtx,
    #[case] roles: Vec<user_roles::Role>,
    #[case] expect_link: bool,
) -> Result<()> {
    let session = factories::session_with_roles(&ctx.state.db, "requester", roles).await?;

    let response = ctx
        .server
        .typed_get(&web::account::Overview {})
        .authorization_bearer(session.secret_token.expose_secret())
        .await;

    response.assert_status_ok();
    let bot_url = web::admin::BotList {}.to_string();
    assert_eq!(response.text().contains(&bot_url), expect_link);

    Ok(())
}

/// Non-admins may not see or create bots
#[rstest]
#[tokio::test]
async fn test_bot_forbidden_for_non_admins(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let session_of_unauthorized_user = factories::session_with_roles(
        &ctx.state.db,
        "non-authorized-user",
        vec![user_roles::Role::PackageMaintainer],
    )
    .await?;
    let bot = factories::bot(&ctx.state.db, "some-bot").await?;

    // Listing the bots should fail.
    let response = ctx
        .server
        .typed_get(&web::admin::BotList {})
        .authorization_bearer(session_of_unauthorized_user.secret_token.expose_secret())
        .await;
    response.assert_status_forbidden();

    // Creating a bot should fail.
    let response = ctx
        .server
        .typed_post(&web::admin::BotCreate {})
        .form(&CreateBot {
            username: "new-bot".to_string(),
        })
        .authorization_bearer(session_of_unauthorized_user.secret_token.expose_secret())
        .await;
    response.assert_status_forbidden();
    assert!(
        !queries::users::by_username("new-bot".to_string())
            .exists(&ctx.state.db)
            .await?
    );

    // Regenerating the bot token should fail.
    let response = ctx
        .server
        .typed_post(&web::admin::BotTokenRegenerate {
            user_id: bot.id.to_string(),
        })
        .authorization_bearer(session_of_unauthorized_user.secret_token.expose_secret())
        .await;
    response.assert_status_forbidden();
    assert!(
        !queries::sessions::by_user_id(bot.id)
            .exists(&ctx.state.db)
            .await?
    );

    Ok(())
}

/// Creating a bot creates the user, its role and a usable token
#[rstest]
#[tokio::test]
async fn test_bot_create(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let response = ctx
        .server
        .typed_post(&web::admin::BotCreate {})
        .form(&CreateBot {
            username: "new-bot".to_string(),
        })
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    response.assert_status_ok();
    response.assert_text_contains("new-bot");

    let bot_id = queries::users::by_username("new-bot".to_string())
        .one(&ctx.state.db)
        .await?
        .expect("bot should exist")
        .id
        .0;
    let bot = queries::users::bot_by_id(bot_id)
        .one(&ctx.state.db)
        .await?
        .expect("bot should have the bot role");

    // The bot should have the bot user role.
    let roles: Vec<user_roles::Role> = user_roles::Entity::find()
        .filter(user_roles::COLUMN.user_id.eq(bot.id))
        .all(&ctx.state.db)
        .await?
        .into_iter()
        .map(|model| model.role)
        .collect();
    assert_eq!(roles, vec![user_roles::Role::Bot]);

    // There should always be a session created alongside the bot user.
    let session = queries::sessions::by_user_id(bot.id)
        .one(&ctx.state.db)
        .await?
        .expect("bot should have a session");
    assert_eq!(session.client_type, ClientType::Bot);
    // We expect to see the token printed on the page...
    response.assert_text_contains(session.secret_token.expose_secret());

    // ...and that token authenticates us as the bot.
    let response = ctx
        .server
        .typed_get(&api::users::AuthenticatedUser {})
        .authorization_bearer(session.secret_token.expose_secret())
        .await;
    response.assert_status_ok();
    let user: api::users::User = response.json();
    assert_eq!(user.username, "new-bot");
    assert_eq!(user.user_roles, vec![api::users::Role::Bot]);

    Ok(())
}

/// Invalid input is rejected
#[rstest]
#[case::too_short("ab".to_string())]
#[case::too_long("a".repeat(256))]
#[tokio::test]
async fn test_bot_create_invalid_input(
    #[future(awt)] ctx: TestCtx,
    #[case] username: String,
) -> Result<()> {
    let response = ctx
        .server
        .typed_post(&web::admin::BotCreate {})
        .form(&CreateBot {
            username: username.clone(),
        })
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    response.assert_status_unprocessable_entity();
    assert_eq!(
        queries::users::by_username(username)
            .count(&ctx.state.db)
            .await?,
        0
    );

    Ok(())
}

/// Regenerating a bot's token deletes all previous tokens and creates a new one
#[rstest]
#[tokio::test]
async fn test_bot_token_regenerate(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let bot = factories::bot(&ctx.state.db, "some-bot").await?;

    // Create the initial session.
    let response = ctx
        .server
        .typed_post(&web::admin::BotTokenRegenerate {
            user_id: bot.id.to_string(),
        })
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;
    response.assert_status_ok();
    let old_session = queries::sessions::by_user_id(bot.id)
        .one(&ctx.state.db)
        .await?
        .expect("bot should have a session");
    let old_token = old_session.secret_token.expose_secret().to_string();

    // Now regenerate the session.
    let response = ctx
        .server
        .typed_post(&web::admin::BotTokenRegenerate {
            user_id: bot.id.to_string(),
        })
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;
    response.assert_status_ok();

    let new_session = queries::sessions::by_user_id(bot.id)
        .one(&ctx.state.db)
        .await?
        .expect("bot should have a session");
    let new_token = new_session.secret_token.expose_secret().to_string();
    assert_ne!(old_token, new_token);

    // The previous token does not authenticate anymore
    let response = ctx
        .server
        .typed_get(&api::users::AuthenticatedUser {})
        .authorization_bearer(&old_token)
        .await;
    response.assert_status_unauthorized();

    Ok(())
}

/// Tokens can only be created for bots, not for humans or unknown ids
#[rstest]
#[tokio::test]
async fn test_bot_token_create_not_a_bot(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let oidc_user = factories::oidc_user(&ctx.state.db, "oidc-user").await?;
    let unknown_user = Uuid::new_v4();

    for user_id in [oidc_user.id.0, unknown_user] {
        let response = ctx
            .server
            .typed_post(&web::admin::BotTokenRegenerate {
                user_id: user_id.to_string(),
            })
            .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
            .await;
        response.assert_status_not_found();
    }

    assert_eq!(
        queries::sessions::by_user_id(oidc_user.id)
            .count(&ctx.state.db)
            .await?,
        0
    );

    Ok(())
}

use std::time::Duration;

use crate::web;
use color_eyre::Result;
use color_eyre::eyre::{Context, ContextCompat};
use rstest::rstest;
use sea_orm::TransactionTrait;
use thirtyfour::{By, prelude::ElementQueryable};
use uuid::Uuid;

use crate::entities::sessions::ClientType;
use crate::queries;
use crate::tests::test_ctx::{CookieJarExt, TestCtx, TestCtxBuilder, ctx};

/// Test the full logout flow really invalidates the session
#[tokio::test]
async fn test_e2e_account_logout() -> Result<()> {
    // Exercise the whole OIDC logout process using a real browser
    let ctx_with_oidc = TestCtxBuilder::new()
        .with_http_transport()
        .with_geckodriver()
        .with_authelia()
        .build()
        .await;

    let c = ctx_with_oidc.thirtyfour_client.clone().unwrap();

    // Start the login process
    c.goto(
        ctx_with_oidc
            .server_url
            .join(&web::oidc::StartLogin {}.to_string())?
            .to_string(),
    )
    .await?;

    // Wait for username field to appear
    let username = "testuser";
    let username_field = c
        .query(By::Id("username-textfield"))
        .wait(Duration::from_secs(5), Duration::from_secs(1))
        .first()
        .await?;
    username_field.send_keys(username).await?;
    c.find(By::Id("password-textfield"))
        .await?
        .send_keys("testpassword")
        .await?;
    c.find(By::Id("sign-in-button")).await?.click().await?;

    // Wait for authorization button to appear and be clickable
    c.query(By::Id("openid-consent-accept"))
        .and_clickable()
        .first()
        .await?
        .click()
        .await?;

    // Wait, then check if we are logged in
    let content = c
        .query(By::Id("navbar-buildbtw"))
        .wait(Duration::from_secs(5), Duration::from_secs(1))
        .first()
        .await?;
    let text = content.text().await?.to_string();
    assert!(
        text.contains(username.to_string().as_str()),
        "expected to show a logged in user",
    );

    let url = c.current_url().await?.to_string();
    assert!(
        url.starts_with(ctx_with_oidc.server_url.as_str()),
        "expected {url} to start with the URL of buildbtw's authorized page"
    );

    // Extract the session secret token
    let private_jar = &ctx_with_oidc
        .private_cookie_jar_from_thirtyfour(c.get_all_cookies().await?.as_ref())
        .wrap_err("Failed to decrypt cookies")?;
    let session_secret_token = private_jar
        .get(crate::from_request::auth_user::SESSION_SECRET_TOKEN_COOKIE_NAME)
        .wrap_err("Failed to get session if from decrypt cookie")?
        .value()
        .to_owned();

    // Check if the session has been added to the database
    let session_record = queries::sessions::by_secret_token(session_secret_token.into())
        .one(&ctx_with_oidc.state.db)
        .await?;
    assert!(
        session_record.is_some(),
        "expected a session record to exist after login",
    );

    // Logout
    c.goto(
        ctx_with_oidc
            .server_url
            .join(&web::account::Logout {}.to_string())?
            .to_string(),
    )
    .await?;

    // Check if we are logged out
    let content = c
        .query(By::Id("navbar-buildbtw"))
        .wait(Duration::from_secs(5), Duration::from_secs(1))
        .first()
        .await?;
    let text = content.text().await?.to_string();
    assert!(
        !text.contains(format!("Logged in as {username}").as_str()),
        "expected to not show a logged in user",
    );

    // Check if the session cookie got removed
    let session_cookie = c
        .get_named_cookie(crate::from_request::auth_user::SESSION_SECRET_TOKEN_COOKIE_NAME)
        .await
        .ok();
    assert!(
        session_cookie.is_none(),
        "expected a session cookie to not exist after logout",
    );

    // Check if the session has been removed from the database
    let session_record = queries::sessions::by_id(session_record.unwrap().id.into())
        .one(&ctx_with_oidc.state.db)
        .await?;
    assert!(
        session_record.is_none(),
        "expected a session record to not exist after logout",
    );

    c.quit().await?;

    Ok(())
}

/// Test session list endpoint lists existing sessions
#[rstest]
#[tokio::test]
async fn test_session_list(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let db = &ctx.state.db;
    let tx = db.begin().await?;

    // Create a valid user
    let create = crate::input::users::ValidatedCreate::try_new(crate::input::users::Create {
        oidc_id: "OIDC_ID".to_string(),
        username: "username".to_string(),
    })?;
    let user = queries::users::upsert(create, None).exec(&tx).await?;

    // Create our session we use for the requests
    let session = queries::sessions::insert(user.last_insert_id.into(), ClientType::Web)
        .exec_with_returning(&tx)
        .await?;

    // Create another session for our user and see if it will be listed
    let another_session = queries::sessions::insert(user.last_insert_id.into(), ClientType::Web)
        .exec(&tx)
        .await?;

    tx.commit().await?;

    // Create cookie jar with the current session
    let private_jar = ctx.private_cookie_jar();
    let private_jar = crate::from_request::auth_user::save_in_cookie_jar(
        &session.secret_token,
        private_jar,
        None,
    );
    let cookies = private_jar.to_encrypted_cookie_jar()?;

    // Request the endpoint to test
    let response = ctx
        .server
        .typed_get(&web::account::SessionList {})
        .add_cookies(cookies)
        .await;

    response.assert_status_ok();
    response.assert_header("content-type", "text/html; charset=utf-8");

    response.assert_text_contains(session.id.to_string());
    response.assert_text_contains(another_session.last_insert_id.0.to_string());

    Ok(())
}

/// Test session revoke endpoint kills the session
#[rstest]
#[tokio::test]
async fn test_session_revoke(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let db = &ctx.state.db;

    // Create a valid user
    let create = crate::input::users::ValidatedCreate::try_new(crate::input::users::Create {
        oidc_id: "OIDC_ID".to_string(),
        username: "username".to_string(),
    })?;
    let user = queries::users::upsert(create, None).exec(db).await?;

    // Create our session we use for the requests
    let session = queries::sessions::insert(user.last_insert_id.into(), ClientType::Web)
        .exec_with_returning(db)
        .await?;

    // Create cookie jar with the current session
    let private_jar = ctx.private_cookie_jar();
    let private_jar = crate::from_request::auth_user::save_in_cookie_jar(
        &session.secret_token,
        private_jar,
        None,
    );
    let cookies = private_jar.to_encrypted_cookie_jar()?;

    // Request the endpoint to test
    let response = ctx
        .server
        .typed_get(&web::account::SessionRevoke {
            session_id: session.id.to_string(),
        })
        .add_cookies(cookies.clone())
        .await;
    response.assert_status_see_other();

    // Check that we are logged out
    let response = ctx
        .server
        .typed_get(&web::index::Index {})
        .add_cookies(cookies)
        .await;
    response.assert_status_ok();
    response.assert_text_contains("Sign in");

    // Check if the session has been removed from the database
    let session_record = queries::sessions::by_id(session.id.into()).one(db).await?;
    assert!(
        session_record.is_none(),
        "expected a session record to not exist after revoke",
    );

    Ok(())
}

/// Test session revoke endpoint kills another session
#[rstest]
#[tokio::test]
async fn test_session_revoke_other_session(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let db = &ctx.state.db;

    // Create a valid user
    let create = crate::input::users::ValidatedCreate::try_new(crate::input::users::Create {
        oidc_id: "OIDC_ID".to_string(),
        username: "test_username".to_string(),
    })?;
    let user = queries::users::upsert(create, None).exec(db).await?;

    // Create our session we use for the requests
    let session = queries::sessions::insert(user.last_insert_id.into(), ClientType::Web)
        .exec_with_returning(db)
        .await?;

    // Create another session we want to kill
    let other_session = queries::sessions::insert(user.last_insert_id.into(), ClientType::Web)
        .exec(db)
        .await?;
    let other_session_id = other_session.last_insert_id.0;

    // Create cookie jar with the current session
    let private_jar = ctx.private_cookie_jar();
    let private_jar = crate::from_request::auth_user::save_in_cookie_jar(
        &session.secret_token,
        private_jar,
        None,
    );
    let cookies = private_jar.to_encrypted_cookie_jar()?;

    // Request the endpoint to test
    let response = ctx
        .server
        .typed_get(&web::account::SessionRevoke {
            session_id: other_session_id.to_string(),
        })
        .add_cookies(cookies.clone())
        .await;
    response.assert_status_see_other();

    // Check that we are not logged out
    let response = ctx
        .server
        .typed_get(&web::index::Index {})
        .add_cookies(cookies)
        .await;
    response.assert_status_ok();
    response.assert_text_contains("test_username");

    // Check if our session has been removed from the database
    let session_record = queries::sessions::by_id(session.id.into()).one(db).await?;
    assert!(
        session_record.is_some(),
        "expected our session record to exist",
    );

    // Check if the other session has been removed from the database
    let session_record = queries::sessions::by_id(other_session_id).one(db).await?;
    assert!(
        session_record.is_none(),
        "expected other session record to not exist",
    );

    Ok(())
}

/// Check that users cannot revoke sessions from other users
#[rstest]
#[tokio::test]
async fn test_session_revoke_cannot_revoke_other_user_session(
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    let db = &ctx.state.db;

    // Create user A
    let create_user_a =
        crate::input::users::ValidatedCreate::try_new(crate::input::users::Create {
            oidc_id: "OIDC_ID_USER_A".to_string(),
            username: "user_a".to_string(),
        })?;
    let user_a = queries::users::upsert(create_user_a, None).exec(db).await?;

    // Create user B
    let create_user_b =
        crate::input::users::ValidatedCreate::try_new(crate::input::users::Create {
            oidc_id: "OIDC_ID_USER_B".to_string(),
            username: "user_b".to_string(),
        })?;
    let user_b = queries::users::upsert(create_user_b, None).exec(db).await?;

    // Create session for user A (we'll authenticate as user A)
    let user_a_session = queries::sessions::insert(user_a.last_insert_id.into(), ClientType::Web)
        .exec_with_returning(db)
        .await?;

    // Create session for user B (we'll try to revoke this)
    let user_b_session = queries::sessions::insert(user_b.last_insert_id.into(), ClientType::Web)
        .exec_with_returning(db)
        .await?;

    // Create cookie jar with user A's session
    let private_jar = ctx.private_cookie_jar();
    let private_jar = crate::from_request::auth_user::save_in_cookie_jar(
        &user_a_session.secret_token,
        private_jar,
        None,
    );
    let cookies = private_jar.to_encrypted_cookie_jar()?;

    // Try to revoke user B's session while authenticated as user A
    let response = ctx
        .server
        .typed_get(&web::account::SessionRevoke {
            session_id: user_b_session.id.to_string(),
        })
        .add_cookies(cookies.clone())
        .await;
    response.assert_status_forbidden();

    // Verify user B's session still exists
    let user_b_session_record = queries::sessions::by_id(user_b_session.id.into())
        .one(db)
        .await?;
    assert!(
        user_b_session_record.is_some(),
        "expected user B's session to still exist",
    );

    Ok(())
}

/// Check that revoking a non-existent session returns 404
#[rstest]
#[tokio::test]
async fn test_session_cannot_revoke_nonexistent(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let db = &ctx.state.db;

    // Create user
    let create_user = crate::input::users::ValidatedCreate::try_new(crate::input::users::Create {
        oidc_id: "OIDC_ID_USER_A".to_string(),
        username: "user_a".to_string(),
    })?;
    let user = queries::users::upsert(create_user, None).exec(db).await?;

    // Create session
    let session = queries::sessions::insert(user.last_insert_id.into(), ClientType::Web)
        .exec_with_returning(db)
        .await?;

    // Create cookie jar with user A's session
    let private_jar = ctx.private_cookie_jar();
    let private_jar = crate::from_request::auth_user::save_in_cookie_jar(
        &session.secret_token.clone(),
        private_jar,
        None,
    );
    let cookies = private_jar.to_encrypted_cookie_jar()?;

    // Try to revoke nonexistent session
    let response = ctx
        .server
        .typed_get(&web::account::SessionRevoke {
            session_id: Uuid::new_v4().to_string(),
        })
        .add_cookies(cookies.clone())
        .await;
    response.assert_status_forbidden();

    // Verify session still exists
    let user_b_session_record = queries::sessions::by_secret_token(session.secret_token)
        .one(db)
        .await?;
    assert!(
        user_b_session_record.is_some(),
        "expected session to still exist",
    );

    Ok(())
}

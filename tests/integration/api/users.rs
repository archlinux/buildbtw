use axum::http::StatusCode;
use buildbtw::{
    api::{self, users::Role},
    entities::{oidc_identity, user_roles},
    input::users::CreateWithRoles,
    queries,
};
use color_eyre::Result;
use rstest::rstest;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;

use crate::factories;
use crate::test_ctx::{TestCtx, ctx};

/// Get the authenticated user
#[rstest]
#[tokio::test]
async fn test_get_authenticated_user(#[future(awt)] ctx: TestCtx) {
    let response = ctx
        .server
        .typed_get(&api::users::AuthenticatedUser {})
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    response.assert_status_ok();
    let user: api::users::User = response.json();
    assert_eq!(user.username, "admin");
}

/// Creating a new user with roles succeeds
#[rstest]
#[case::package_maintainer(vec![Role::PackageMaintainer])]
#[case::bot(vec![Role::Bot])]
#[case::multiple(vec![Role::Bot, Role::PackageMaintainer])]
#[tokio::test]
async fn test_create_user(#[future(awt)] ctx: TestCtx, #[case] roles: Vec<Role>) -> Result<()> {
    let request = CreateWithRoles {
        username: "somebot".to_string(),
        user_roles: roles.clone(),
    };
    let response = ctx
        .server
        .typed_post(&api::users::CreateUser {})
        .json(&request)
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    response.assert_status_ok();
    let created: api::users::User = response.json();
    assert_eq!(created.username, "somebot");
    assert_eq!(created.user_roles, roles);
    assert!(created.sessions.is_empty());

    let user = queries::users::by_username("somebot".to_string())
        .one(&ctx.state.db)
        .await?
        .expect("created user should exist");

    let db_roles: Vec<Role> = user_roles::Entity::find()
        .filter(user_roles::COLUMN.user_id.eq(user.id))
        .all(&ctx.state.db)
        .await?
        .into_iter()
        .map(|model| model.role.into())
        .collect();
    assert_eq!(db_roles, roles);

    // Users created this way are bots: they must not have an OIDC identity
    let identity = oidc_identity::Entity::find()
        .filter(oidc_identity::COLUMN.user_id.eq(user.id))
        .one(&ctx.state.db)
        .await?;
    assert!(identity.is_none());

    Ok(())
}

/// Only admins may create users
#[rstest]
#[case::package_maintainer(vec![user_roles::Role::PackageMaintainer])]
#[case::bot(vec![user_roles::Role::Bot])]
#[case::no_roles(vec![])]
#[tokio::test]
async fn test_create_user_forbidden_for_non_admins(
    #[future(awt)] ctx: TestCtx,
    #[case] roles: Vec<user_roles::Role>,
) -> Result<()> {
    let session = factories::session_with_roles(&ctx.state.db, "requester", roles).await?;

    let request = CreateWithRoles {
        username: "somebot".to_string(),
        user_roles: vec![Role::Bot],
    };
    let response = ctx
        .server
        .typed_post(&api::users::CreateUser {})
        .json(&request)
        .authorization_bearer(session.secret_token.expose_secret())
        .await;

    response.assert_status_forbidden();

    Ok(())
}

/// Creating a user requires authentication
#[rstest]
#[tokio::test]
async fn test_create_user_unauthenticated(#[future(awt)] ctx: TestCtx) {
    let request = CreateWithRoles {
        username: "somebot".to_string(),
        user_roles: vec![Role::Bot],
    };
    let response = ctx
        .server
        .typed_post(&api::users::CreateUser {})
        .json(&request)
        .await;

    response.assert_status_unauthorized();
}

/// Creating a user with an already taken username returns a conflict
#[rstest]
#[tokio::test]
async fn test_create_user_conflict(#[future(awt)] ctx: TestCtx) {
    let request = CreateWithRoles {
        username: "someuser".to_string(),
        user_roles: vec![Role::Bot],
    };

    for expected_status in [StatusCode::OK, StatusCode::CONFLICT] {
        let response = ctx
            .server
            .typed_post(&api::users::CreateUser {})
            .json(&request)
            .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
            .await;

        response.assert_status(expected_status);
    }
}

/// Invalid input is rejected
#[rstest]
#[case::username_too_short("ab".to_string(), vec![Role::Bot])]
#[case::username_too_long("a".repeat(256), vec![Role::Bot])]
#[case::empty_roles("someuser".to_string(), vec![])]
#[tokio::test]
async fn test_create_user_invalid_input(
    #[future(awt)] ctx: TestCtx,
    #[case] username: String,
    #[case] user_roles: Vec<Role>,
) {
    let request = CreateWithRoles {
        username,
        user_roles,
    };
    let response = ctx
        .server
        .typed_post(&api::users::CreateUser {})
        .json(&request)
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    response.assert_status_unprocessable_entity();
}

/// Unknown role names are rejected when deserializing the request
#[rstest]
#[tokio::test]
async fn test_create_user_unknown_role(#[future(awt)] ctx: TestCtx) {
    let response = ctx
        .server
        .typed_post(&api::users::CreateUser {})
        .json(&json!({"username": "somebot", "user_roles": ["Overlord"]}))
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    response.assert_status_unprocessable_entity();
}

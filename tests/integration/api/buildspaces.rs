use axum_test::TestResponse;
use buildbtw::{
    api::{self, buildspaces::GetBuildspaceResponse},
    buildspace, queries,
};
use color_eyre::eyre::Result;
use rstest::rstest;
use sea_orm::TransactionTrait;
use serde_json::json;

use crate::{
    factories,
    test_ctx::{TestCtx, ctx},
};

fn make_create(
    name: Option<&'static str>,
    changesets: &[(&'static str, &'static str)],
) -> serde_json::Value {
    // Use json instead of a Create struct so we can send invalid data
    json!({
        "name": name,
        "changesets": changesets
            .iter()
            .map(
                |&(repo_slug, branch_name)| json!( {
                    "repo_slug": repo_slug,
                    "branch_name": branch_name,
                }),
            )
            .collect::<Vec<_>>()
    })
}

async fn create_buildspace(ctx: &TestCtx, request: &serde_json::Value) -> TestResponse {
    ctx.server
        .typed_post(&api::buildspaces::CreateBuildspace {})
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .json(request)
        .await
}

/// Verify that we can create a buildspace via the API.
#[rstest]
#[case(Some("buildspace"))]
#[case(None)]
#[tokio::test]
async fn test_create_buildspace_success(
    // Specify buildspace name explicitly, or use the first pkgbase as default.
    #[case] name: Option<&'static str>,
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    // Send request
    let changeset_pkgbase = "pkgbase";
    let request = make_create(name, &[(changeset_pkgbase, "main")]);
    let response = create_buildspace(&ctx, &request).await;

    // Check response
    response.assert_status_ok();
    let body: api::buildspaces::CreateBuildspaceResponse = response.json();
    let expected_name = name.unwrap_or(changeset_pkgbase);
    assert_eq!(body.name.as_ref(), expected_name);

    // Check that the buildspace was written to the db
    let buildspace = queries::buildspaces::by_name(expected_name.parse()?)
        .one(&ctx.state.db)
        .await?
        .expect("buildspace should be persisted in the database");
    assert_eq!(buildspace.name.as_ref(), expected_name);

    // Check that an iteration was created with the correct changesets
    let iteration = queries::iterations::by_sequence(buildspace.id, 1)
        .one(&ctx.state.db)
        .await?
        .expect("iteration not found");
    assert_eq!(iteration.changesets.0.len(), 1);

    assert_eq!(
        iteration
            .changesets
            .into_iter()
            .next()
            .unwrap()
            .branch_name
            .as_str(),
        "main"
    );

    Ok(())
}

/// Check that we can't create a buildspace if not logged in
#[rstest]
#[tokio::test]
async fn test_create_buildspace_unauthorized(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Send request
    let request = make_create(Some("my-buildspace"), &[("libfoo", "main")]);

    let response = ctx
        .server
        .typed_post(&api::buildspaces::CreateBuildspace {})
        .json(&request)
        .await;

    // Check status and error message
    response.assert_status_unauthorized();
    response.assert_text_contains("Unauthorized");
    Ok(())
}

/// Check that we cannot create a buildspace without any changesets
#[rstest]
#[tokio::test]
async fn test_create_buildspace_empty_changesets(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Send request
    let request = make_create(Some("my-buildspace"), &[]);
    let response = create_buildspace(&ctx, &request).await;

    // Check status and error message
    response.assert_status_unprocessable_entity();
    response.assert_text_contains("changesets");
    Ok(())
}

/// Check that we cannot create a buildspace with a name that is already taken
#[rstest]
#[tokio::test]
async fn test_create_buildspace_duplicate_name(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create existing buildspace
    let request = make_create(Some("my-buildspace"), &[("libfoo", "main")]);
    let response = create_buildspace(&ctx, &request).await;
    response.assert_status_ok();

    // Send request
    let response = create_buildspace(&ctx, &request).await;

    // Check status and error message
    response.assert_status_conflict();
    response.assert_text_contains("already exists");
    Ok(())
}

/// Check that we can't create a buildspace with an invalid name
#[rstest]
#[case("")]
#[case("🥴")]
#[tokio::test]
async fn test_create_buildspace_invalid_name(
    #[future(awt)] ctx: TestCtx,
    #[case] name: &'static str,
) -> Result<()> {
    // Send request
    let request = make_create(Some(name), &[]);
    let response = create_buildspace(&ctx, &request).await;

    // Check status and error message
    response.assert_status_unprocessable_entity();
    response.assert_text_contains("name");
    response.assert_text_contains("empty");
    Ok(())
}

/// Check that we can't create a buildspace with invalid characters in the changeset
#[rstest]
#[case("")]
#[case("lemao.git")]
#[case(".sdf-")]
#[case("libsigc++-3.0")]
#[tokio::test]
async fn test_create_buildspace_invalid_changeset(
    #[case] repo_slug: &'static str,
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    // Send request
    let request = make_create(Some("buildspace"), &[(repo_slug, "main")]);
    let response = create_buildspace(&ctx, &request).await;

    // Check status and error message
    response.assert_status_unprocessable_entity();
    response.assert_text_contains("repo_slug");
    Ok(())
}

/// Check that we can't create a buildspace without any changesets
#[rstest]
#[tokio::test]
async fn test_create_buildspace_no_name_no_changesets(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Send request
    let response = create_buildspace(&ctx, &serde_json::json!({})).await;

    // Check status and error message
    response.assert_status_unprocessable_entity();
    response.assert_text_contains("changesets");
    response.assert_text_contains("missing field");
    Ok(())
}

/// Check that we can't create a buildspace where the name taken from the first changeset is already taken
#[rstest]
#[tokio::test]
async fn test_create_buildspace_no_name_duplicate_changeset_slug(
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    // Send request
    let request = make_create(None, &[("libfoo", "main")]);
    let response = create_buildspace(&ctx, &request).await;
    response.assert_status_ok();

    let response = create_buildspace(&ctx, &request).await;

    // Check status and error message
    response.assert_status_conflict();
    response.assert_text_contains("already exists");
    Ok(())
}

/// Check that we can't create a buildspace where the name taken from the first changeset contains invalid chars
#[rstest]
#[tokio::test]
async fn test_create_buildspace_no_name_invalid_changeset_slug(
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    // Send request
    let request = make_create(None, &[("libfoo+++++", "main")]);
    let response = create_buildspace(&ctx, &request).await;

    // Check status and error message
    response.assert_status_unprocessable_entity();
    response.assert_text_contains("changesets[0].repo_slug");
    response.assert_text_contains("special characters");
    Ok(())
}

/// Verify that we can get a buildspace via the API
#[rstest]
#[tokio::test]
async fn test_get_buildspace_success(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace
    let tx = ctx.state.db.begin().await?;
    let buildspace = factories::buildspace(&tx, "buildspace").await?;
    tx.commit().await?;

    // Get the buildspace
    let response = ctx
        .server
        .typed_get(&api::buildspaces::GetBuildspace {
            name: buildspace.name.clone(),
        })
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    // Check response
    response.assert_status_ok();
    let body: GetBuildspaceResponse = response.json();
    assert_eq!(body.id, buildspace.id.0);
    assert_eq!(&body.name, &buildspace.name);
    assert_eq!(body.status, buildspace::Status::Started);

    Ok(())
}

/// Check that getting a non-existent buildspace returns 404
#[rstest]
#[tokio::test]
async fn test_get_buildspace_not_found(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let response = ctx
        .server
        .typed_get(&api::buildspaces::GetBuildspace {
            name: "nonexistent".parse()?,
        })
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    response.assert_status_not_found();
    response.assert_text_contains("Not found");
    Ok(())
}

/// Check that we can't get a buildspace if not logged in
#[rstest]
#[tokio::test]
async fn test_get_buildspace_unauthorized(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let response = ctx
        .server
        .typed_get(&api::buildspaces::GetBuildspace {
            name: "my-buildspace".parse()?,
        })
        .await;

    response.assert_status_unauthorized();
    response.assert_text_contains("Unauthorized");
    Ok(())
}

/// Verify that we can close a buildspace via the API
#[rstest]
#[tokio::test]
async fn test_close_buildspace_success(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace
    let tx = ctx.state.db.begin().await?;
    let buildspace = factories::buildspace(&tx, "my-buildspace").await?;
    tx.commit().await?;

    // Close the buildspace and check response
    let response = ctx
        .server
        .typed_put(&api::buildspaces::CloseBuildspace {
            name: buildspace.name.clone(),
        })
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;
    response.assert_status_ok();

    // Check that closing twice succeeds
    let response = ctx
        .server
        .typed_put(&api::buildspaces::CloseBuildspace {
            name: buildspace.name.clone(),
        })
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;
    response.assert_status_ok();

    // Verify status was updated in the database
    let buildspace = queries::buildspaces::by_name("my-buildspace".parse()?)
        .one(&ctx.state.db)
        .await?
        .expect("buildspace should still exist");
    assert_eq!(buildspace.status, buildspace::Status::Stopped);

    Ok(())
}

/// Check that closing a non-existent buildspace returns 404
#[rstest]
#[tokio::test]
async fn test_close_buildspace_not_found(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let response = ctx
        .server
        .typed_put(&api::buildspaces::CloseBuildspace {
            name: "nonexistent".parse()?,
        })
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    response.assert_status_not_found();
    response.assert_text_contains("Not found");
    Ok(())
}

/// Check that we can't close a buildspace if not logged in
#[rstest]
#[tokio::test]
async fn test_close_buildspace_unauthorized(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let response = ctx
        .server
        .typed_put(&api::buildspaces::CloseBuildspace {
            name: "my-buildspace".parse()?,
        })
        .await;

    response.assert_status_unauthorized();
    response.assert_text_contains("Unauthorized");
    Ok(())
}

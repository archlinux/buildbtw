use color_eyre::{Result, eyre::ContextCompat};
use openidconnect::RefreshToken;
use sea_orm::{ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::{
    db,
    db_fields::{RedactedString, TxtUuid},
    entities::{oidc_identity, users},
    input,
};

/// Create a user with the given OIDC identity, or update an existing one
///
/// If an OIDC identity with the given OIDC id already exists, its refresh token and the owning
/// user's username are updated. Otherwise, a new user and OIDC identity are created.
///
/// This takes a [`db::TxImmediate`] instead of a generic connection because the find-then-create
/// is only race-free inside an immediate transaction. It holds SQLite's only write lock for its
/// whole span so no other transaction can commit between the select and the insert/update.
pub async fn upsert_with_oidc(
    db::TxImmediate(db): &db::TxImmediate,
    create: input::users::ValidatedCreate,
    refresh_token: Option<RefreshToken>,
) -> Result<users::Model> {
    let create = create.into_inner();
    let refresh_token = refresh_token.map(|rt| RedactedString::from(rt.secret()));

    // An `.on_conflict()` upsert isn't possible here: the conflict key (oidc_id) lives in the
    // oidc_identities table while the user row has no unique non-id key that we could use.
    let existing_identity = oidc_identity::Entity::find()
        .filter(oidc_identity::COLUMN.oidc_id.eq(&create.oidc_id))
        .find_both_related(users::Entity)
        .one(db)
        .await?;

    let user = if let Some((identity, user)) = existing_identity {
        // Update the user
        let mut identity: oidc_identity::ActiveModel = identity.into();
        identity.refresh_token = Set(refresh_token);
        oidc_identity::Entity::update(identity).exec(db).await?;

        let mut user: users::ActiveModel = user.into();
        user.username = Set(create.username);
        users::Entity::update(user).exec(db).await?
    } else {
        // Create a new user and OIDC id.
        let user = users::ActiveModel {
            id: Set(Uuid::new_v4().into()),
            created_at: Set(time::OffsetDateTime::now_utc()),
            username: Set(create.username),
        };
        let user = users::Entity::insert(user).exec_with_returning(db).await?;

        let identity = oidc_identity::ActiveModel {
            id: Set(Uuid::new_v4().into()),
            created_at: Set(time::OffsetDateTime::now_utc()),
            user_id: Set(user.id),
            refresh_token: Set(refresh_token),
            oidc_id: Set(create.oidc_id),
        };
        oidc_identity::Entity::insert(identity).exec(db).await?;

        user
    };

    Ok(user)
}

/// Update the refresh token for a user's OIDC identity
pub async fn update_refresh_token(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    new_refresh_token: Option<RefreshToken>,
) -> Result<()> {
    let mut identity: oidc_identity::ActiveModel = oidc_identity::Entity::find()
        .filter(oidc_identity::COLUMN.user_id.eq(TxtUuid::from(user_id)))
        .one(db)
        .await?
        .wrap_err("User has no OIDC identity")?
        .into();

    identity.refresh_token = Set(new_refresh_token.map(|rt| RedactedString::from(rt.secret())));
    oidc_identity::Entity::update(identity).exec(db).await?;

    Ok(())
}

/// Clear the refresh token for a user's OIDC identity
pub async fn clear_refresh_token(db: &impl ConnectionTrait, user_id: Uuid) -> Result<()> {
    update_refresh_token(db, user_id, None).await
}

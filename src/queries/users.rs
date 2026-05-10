use color_eyre::{Result, eyre::eyre};
use openidconnect::RefreshToken;
use sea_orm::{ActiveValue::Set, ConnectionTrait, EntityTrait, Insert, sea_query::OnConflict};
use uuid::Uuid;

use crate::{
    db_fields::{RedactedString, TxtUuid},
    entities::users,
    input,
};

#[must_use]
pub fn upsert(
    create: input::users::ValidatedCreate,
    refresh_token: Option<RefreshToken>,
) -> Insert<users::ActiveModel> {
    let create = create.into_inner();
    let model = users::ActiveModel {
        oidc_id: Set(create.oidc_id),
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        username: Set(create.username.clone()),
        refresh_token: Set(refresh_token.map(|rt| RedactedString::from(rt.secret()))),
    };

    users::Entity::insert(model).on_conflict(
        OnConflict::column(users::COLUMN.oidc_id)
            .update_column(users::COLUMN.username)
            .update_column(users::COLUMN.refresh_token)
            .to_owned(),
    )
}

/// Update the refresh token for a user
pub async fn update_refresh_token(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    new_refresh_token: Option<RefreshToken>,
) -> Result<()> {
    let mut user: users::ActiveModel = users::Entity::find_by_id(TxtUuid::from(user_id))
        .one(db)
        .await?
        .ok_or_else(|| eyre!("User not found"))?
        .into();

    user.refresh_token = Set(new_refresh_token.map(|rt| RedactedString::from(rt.secret())));
    users::Entity::update(user).exec(db).await?;

    Ok(())
}

/// Clear the refresh token for a user
pub async fn clear_refresh_token(db: &impl ConnectionTrait, user_id: Uuid) -> Result<()> {
    update_refresh_token(db, user_id, None).await
}

use color_eyre::{Result, eyre::eyre};
use openidconnect::RefreshToken;
use sea_orm::{ActiveValue::Set, ConnectionTrait, EntityTrait, Insert, sea_query::OnConflict};
use uuid::Uuid;

use crate::{db_fields::TextUuid, entities::users, input};

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
        refresh_token: Set(refresh_token.map(|rt| rt.secret().to_string())),
    };

    users::Entity::insert(model).on_conflict(
        OnConflict::column(users::Column::OidcId)
            .update_columns([users::Column::Username, users::Column::RefreshToken])
            .to_owned(),
    )
}

/// Update the refresh token for a user
pub async fn update_refresh_token(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    new_refresh_token: Option<RefreshToken>,
) -> Result<()> {
    let mut user: users::ActiveModel = users::Entity::find_by_id(TextUuid::from(user_id))
        .one(db)
        .await?
        .ok_or_else(|| eyre!("User not found"))?
        .into();

    user.refresh_token = Set(new_refresh_token.map(|rt| rt.secret().to_string()));
    users::Entity::update(user).exec(db).await?;

    Ok(())
}

/// Clear the refresh token for a user
pub async fn clear_refresh_token(db: &impl ConnectionTrait, user_id: Uuid) -> Result<()> {
    update_refresh_token(db, user_id, None).await
}

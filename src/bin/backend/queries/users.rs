use sea_orm::{ActiveValue::Set, EntityTrait, Insert, sea_query::OnConflict};
use uuid::Uuid;

use crate::{entities::users, input};

pub fn upsert(create: input::users::ValidatedCreate) -> Insert<users::ActiveModel> {
    let create = create.into_inner();
    let model = users::ActiveModel {
        oidc_id: Set(create.oidc_id),
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        username: Set(create.username.clone()),
        role: Set(users::Role::None),
    };

    users::Entity::insert(model).on_conflict(
        OnConflict::column(users::Column::OidcId)
            .update_column(users::Column::Username)
            .value(users::Column::Username, create.username)
            .to_owned(),
    )
}

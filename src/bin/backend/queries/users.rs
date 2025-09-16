use sea_orm::{ActiveValue::Set, EntityTrait, Insert, sea_query::OnConflict};
use uuid::Uuid;

use crate::entities::users;

pub fn upsert(oidc_id: String) -> Insert<users::ActiveModel> {
    let model = users::ActiveModel {
        oidc_id: Set(oidc_id),
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
    };

    users::Entity::insert(model).on_conflict(
        OnConflict::column(users::Column::OidcId)
            .do_nothing()
            .to_owned(),
    )
}

use garde::Validate;
use nutype::nutype;
use serde::{Deserialize, Serialize};

use crate::api::users::Role;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateWithRoles {
    // <https://gitlab.archlinux.org/archlinux/infrastructure/-/blob/388f7b7efb4649b3d20ff17d4bbfbdc359182ae8/tf-stage2/keycloak.tf#L148>
    #[garde(length(min = 3, max = 255))]
    pub username: String,

    #[garde(length(min = 1))]
    pub user_roles: Vec<Role>,
}

// TODO: check if we can use garde's `Valid` type instead: https://docs.rs/garde/latest/garde/validate/struct.Valid.html
#[nutype(derive(Into, TryFrom), validate(with = CreateWithRoles::validate, error = garde::Report))]
pub struct ValidatedCreateWithRoles(CreateWithRoles);

#[derive(Debug, Validate)]
pub struct CreateWithOidc {
    #[garde(length(max = 255))]
    pub oidc_id: String,
    // <https://gitlab.archlinux.org/archlinux/infrastructure/-/blob/388f7b7efb4649b3d20ff17d4bbfbdc359182ae8/tf-stage2/keycloak.tf#L148>
    #[garde(length(min = 3, max = 255))]
    pub username: String,
}

// TODO: check if we can use garde's `Valid` type instead: https://docs.rs/garde/latest/garde/validate/struct.Valid.html
#[nutype(derive(Into), validate(with = CreateWithOidc::validate, error = garde::Report))]
pub struct ValidatedCreateWithOidc(CreateWithOidc);

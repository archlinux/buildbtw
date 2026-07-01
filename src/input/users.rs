use garde::Validate;
use nutype::nutype;

#[derive(Debug, Validate)]
pub struct Create {
    #[garde(length(max = 255))]
    pub oidc_id: String,
    // <https://gitlab.archlinux.org/archlinux/infrastructure/-/blob/388f7b7efb4649b3d20ff17d4bbfbdc359182ae8/tf-stage2/keycloak.tf#L148>
    #[garde(length(min = 3, max = 255))]
    pub username: String,
}

// TODO: check if we can use garde's `Valid` type instead: https://docs.rs/garde/latest/garde/validate/struct.Valid.html
#[nutype(derive(Into), validate(with = Create::validate, error = garde::Report))]
pub struct ValidatedCreate(Create);

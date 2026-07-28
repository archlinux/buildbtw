//! Types for dealing with buildspace-specific data.

use nutype::nutype;
use serde::{Deserialize, Serialize};

use crate::utils::slugify;

/// A buildspace slug to represent a buildspace by name with type safety.
///
/// Wraps a [`String`] and automatically sanatizes it through [`crate::utils::slugify`].
#[nutype(
    sanitize(with = slugify),
    validate(with = validate_buildspace_slug, error = garde::Error),
    derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, AsRef, Deref, TryFrom, FromStr, Display),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unchecked(sea_orm::DeriveValueType)
)]
pub struct Slug(String);

fn validate_buildspace_slug(input: &str) -> Result<(), garde::Error> {
    if input.is_empty() {
        return Err(garde::Error::new("May not be empty"));
    }

    Ok(())
}

/// States of a buildspace
///
/// This is intentionally separate from [crate::entities::iterations::Status] because:
/// 1. Stopped buildspaces don't automatically receive new iterations. Whether or not it receives new iterations is the concern of a buildspace, so it would not make sense to use the iteration status for it.
/// 2. We don't want to erase potential graph calculation errors as those can be useful info even when a buildspace is stopped
///
/// The drawback of this design is that we have lots of different statuses (buildspace, iteration, and build status) which might interact in surprising ways.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    derive_more::Display,
    derive_more::FromStr,
    sea_orm::DeriveValueType,
    Serialize,
    Deserialize,
    strum::EnumIter,
    Hash,
)]
#[sea_orm(value_type = "String")]
pub enum Status {
    Started,
    Stopped,
}

impl Status {
    #[must_use]
    pub fn symbol(&self) -> &str {
        match self {
            Status::Started => "▶️",
            Status::Stopped => "⏹️",
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("test\nit   now!", "test-it-now")]
    #[case("Æúű--cool?", "cool")]
    #[case("foo/../../bar", "foo-bar")]
    #[case("already-a-slug", "already-a-slug")]
    fn buildspace_slug_valid(#[case] s: &str, #[case] expected: &str) {
        let slug = Slug::try_new(s).unwrap();
        assert_eq!(slug.as_ref(), expected, "'{s}' should be slugified");
        // check construction is idempotent
        assert_eq!(Slug::try_new(expected).unwrap(), slug);
    }

    #[rstest]
    #[case("")]
    #[case("-.-")]
    #[case("..")]
    #[case("Æúű")]
    fn buildspace_slug_invalid(#[case] s: &str) {
        assert!(Slug::try_new(s).is_err(), "'{s}' should be an invalid slug");
    }
}

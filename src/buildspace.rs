//! Types for dealing with buildspace-specific data.

use nutype::nutype;

use crate::utils::slugify;

/// A buildspace slug to represent a buildspace by name with type safety.
///
/// Wraps a [`String`] and automatically sanatizes it through [`crate::utils::slugify`].
#[nutype(
    sanitize(with = slugify),
    validate(not_empty),
    derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, AsRef, Deref, TryFrom, FromStr, Display),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unchecked(sea_orm::DeriveValueType)
)]
pub struct BuildspaceSlug(String);

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("test\nit   now!", "test-it-now")]
    #[case("Æúű--cool?", "cool")]
    #[case("foo/../../bar", "foo-bar")]
    #[case("already-a-slug", "already-a-slug")]
    fn buildspace_slug_valid(#[case] s: &str, #[case] expected: &str) {
        let slug = BuildspaceSlug::try_new(s).unwrap();
        assert_eq!(slug.as_ref(), expected, "'{s}' should be slugified");
        // check construction is idempotent
        assert_eq!(BuildspaceSlug::try_new(expected).unwrap(), slug);
    }

    #[rstest]
    #[case("")]
    #[case("-.-")]
    #[case("..")]
    #[case("Æúű")]
    fn buildspace_slug_invalid(#[case] s: &str) {
        assert!(
            BuildspaceSlug::try_new(s).is_err(),
            "'{s}' should be an invalid slug"
        );
    }
}

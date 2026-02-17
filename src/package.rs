//! Newtypes for dealing with package-related values.

use nutype::nutype;

use crate::regex;

/// A package source repository name.
///
/// This newtype wrapper provides type safety when working with repository
/// references in the build system.
#[nutype(
    // See https://docs.gitlab.com/user/reserved_names/#rules-for-usernames-project-and-group-names-and-slugs
    validate(predicate = validate_repository_name),
    derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, AsRef, Deref, TryFrom),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unsafe(sea_orm::FromJsonQueryResult)
)]
pub struct RepositorySlug(String);

fn validate_repository_name(name: &str) -> bool {
    #![expect(
        clippy::case_sensitive_file_extension_comparisons,
        reason = "Clippy doesn't recognize we've fixed this."
    )]
    let lowercase_name = name.to_ascii_lowercase();
    // set of all allowed chars, no matter the position
    regex!("^[a-zA-Z0-9_\\.\\-\\+]+$").is_match(name)
        // subset of allowed outer chars
        && regex!("^[a-zA-Z0-9].*[a-zA-Z0-9]$").is_match(name)
        // no consecutive special chars
        && !regex!("[\\-\\+\\_]{2,}").is_match(name)
        && !lowercase_name.ends_with(".git")
        && !lowercase_name.ends_with(".atom")
}

#[cfg(test)]
mod tests {
    use super::RepositorySlug;
    use rstest::rstest;

    #[rstest]
    #[case("a_z.A-Z+09a")]
    fn repository_slug_valid(#[case] slug: &str) {
        assert!(
            RepositorySlug::try_new(slug).is_ok(),
            "'{slug}' should be a valid slug"
        );
    }

    #[rstest]
    // May not end with ".git" or ".atom"
    #[case("lemao.git")]
    #[case("lemao.atom")]
    // Needs letter or number at the start and end
    #[case(".sdf-")]
    #[case("+sdf_")]
    #[case("afl++")]
    // No consecutive special chars
    #[case("libsigc++-3.0")]
    #[case("a--b")]
    #[case("a__b")]
    #[case("a+_b")]
    fn repository_slug_invalid(#[case] slug: &str) {
        assert!(
            RepositorySlug::try_new(slug).is_err(),
            "'{slug}' should be an invalid slug"
        );
    }
}

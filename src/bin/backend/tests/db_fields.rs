use rstest::rstest;

use crate::db_fields::RepositorySlug;

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

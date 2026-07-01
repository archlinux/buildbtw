use serde::{Deserialize, Serialize};

use crate::{buildspace::BuildspaceSlug, git, input::garde_report, package};

/// A changeset in a create buildspace request.
/// It's a separate struct so we can add an opinionated default
/// value for the branch name.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateChangeset {
    pub repo_slug: package::RepositorySlug,

    // Can't use serde's default attr here because it doesn't work
    // with `null` values: https://github.com/serde-rs/serde/issues/1098
    pub branch_name: Option<git::BranchName>,
}

impl From<CreateChangeset> for git::Changeset {
    fn from(
        CreateChangeset {
            repo_slug,
            branch_name,
        }: CreateChangeset,
    ) -> Self {
        git::Changeset {
            repo_slug,
            branch_name: branch_name.unwrap_or_else(git::BranchName::main),
        }
    }
}

/// Input for creating a new buildspace.
#[derive(Debug, Serialize, Deserialize)]
pub struct Create {
    /// If left out, will use the repo name of the first changeset.
    pub name: Option<BuildspaceSlug>,

    pub changesets: Vec<CreateChangeset>,
}

/// Input, validated and transformed to use our newtypes.
#[derive(Debug)]
pub struct ValidatedCreate {
    pub name: BuildspaceSlug,
    pub changesets: git::Changesets,
}

impl TryFrom<Create> for ValidatedCreate {
    type Error = garde::Report;

    /// Validate and transform the input to our newtypes, returning a garde Report if anything fails.
    fn try_from(value: Create) -> Result<Self, Self::Error> {
        // Convert changesets to our newtype
        let changesets: git::Changesets = value
            .changesets
            .into_iter()
            .map(git::Changeset::from)
            .collect::<Vec<_>>()
            .into();

        // Validate that changesets are not empty (a buildspace without changesets would build nothing)
        let first_changeset = changesets.0.first().ok_or_else(|| {
            garde_report(
                garde::Path::new("changesets"),
                garde::Error::new("must not be empty"),
            )
        })?;

        // Use pkgbase of the first changeset as default name
        let name = value
            .name
            .unwrap_or(
                first_changeset
                    .repo_slug
                    .to_string()
                    .try_into()
                    .map_err(|e| {
                        garde_report(garde::Path::new("changesets").join(0).join("repo_slug"), e)
                    })?,
            );

        Ok(ValidatedCreate { name, changesets })
    }
}

use serde::{Deserialize, Serialize};

use crate::{buildspace, git, input::garde_report};

/// Input for creating a new buildspace.
#[derive(Debug, Serialize, Deserialize)]
pub struct Create {
    /// If left out, will use the repo name of the first changeset.
    pub name: Option<buildspace::Slug>,

    pub changesets: git::Changesets,
}

/// Input, validated and transformed to use our newtypes.
#[derive(Debug)]
pub struct ValidatedCreate {
    pub name: buildspace::Slug,
    pub changesets: git::Changesets,
}

impl TryFrom<Create> for ValidatedCreate {
    type Error = garde::Report;

    /// Validate and transform the input to our newtypes, returning a garde Report if anything fails.
    fn try_from(value: Create) -> Result<Self, Self::Error> {
        // Validate that changesets are not empty (a buildspace without changesets would build nothing)
        let first_changeset = value.changesets.0.first().ok_or_else(|| {
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

        Ok(ValidatedCreate {
            name,
            changesets: value.changesets,
        })
    }
}

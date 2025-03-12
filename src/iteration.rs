use crate::{
    build_set_graph::{self, calculate_packages_to_be_built, diff, BuildSetGraph},
    BuildNamespace, BuildNamespaceStatus, BuildSetIteration,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NewIterationReason {
    FirstIteration,
    OriginChangesetsChanged,
    BuildSetGraphChanged { diff: Box<build_set_graph::Diff> },
    CreatedByUser,
}

pub enum NewBuildIterationResult {
    NoNewIterationNeeded,
    NewIterationNeeded {
        packages_to_build: BuildSetGraph,
        reason: NewIterationReason,
    },
}

pub async fn new_build_set_iteration_is_needed(
    namespace: &BuildNamespace,
    iterations: &[BuildSetIteration],
) -> Result<NewBuildIterationResult> {
    if namespace.status == BuildNamespaceStatus::Cancelled {
        return Ok(NewBuildIterationResult::NoNewIterationNeeded);
    }

    let packages_to_build = calculate_packages_to_be_built(namespace).await?;

    let previous_iteration = if let Some(it) = iterations.last() {
        it
    } else {
        return Ok(NewBuildIterationResult::NewIterationNeeded {
            packages_to_build,
            reason: NewIterationReason::FirstIteration,
        });
    };

    if previous_iteration.origin_changesets != namespace.current_origin_changesets {
        return Ok(NewBuildIterationResult::NewIterationNeeded {
            packages_to_build,
            reason: NewIterationReason::OriginChangesetsChanged,
        });
    }

    let diff = diff(&previous_iteration.packages_to_be_built, &packages_to_build);
    if !diff.is_empty() {
        return Ok(NewBuildIterationResult::NewIterationNeeded {
            packages_to_build,
            reason: NewIterationReason::BuildSetGraphChanged {
                diff: Box::new(diff),
            },
        });
    }

    Ok(NewBuildIterationResult::NoNewIterationNeeded)
}

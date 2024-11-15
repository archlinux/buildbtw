use crate::{
    build_set_graph::{build_set_graph_eq, calculate_packages_to_be_built, BuildSetGraph},
    BuildNamespace,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum NewIterationReason {
    FirstIteration,
    OriginChangesetsChanged,
    BuildSetGraphChanged,
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
) -> Result<NewBuildIterationResult> {
    let packages_to_build = calculate_packages_to_be_built(namespace).await?;

    let previous_iteration = if let Some(it) = namespace.iterations.last() {
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

    if !build_set_graph_eq(&packages_to_build, &previous_iteration.packages_to_be_built) {
        return Ok(NewBuildIterationResult::NewIterationNeeded {
            packages_to_build,
            reason: NewIterationReason::BuildSetGraphChanged,
        });
    }

    Ok(NewBuildIterationResult::NoNewIterationNeeded)
}

use std::collections::{HashMap, HashSet};

use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};

use crate::{
    BuildNamespace, BuildNamespaceStatus, BuildSetIteration,
    build_set_graph::{self, BuildSetGraph, calculate_packages_to_be_built, diff_graphs},
    source_info::ConcreteArchitecture,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NewIterationReason {
    FirstIteration,
    OriginChangesetsChanged,
    BuildSetGraphChanged { diff: Box<IterationDiff> },
    CreatedByUser,
}

impl NewIterationReason {
    pub fn short_description(&self) -> &'static str {
        match self {
            NewIterationReason::FirstIteration => "First iteration",
            NewIterationReason::OriginChangesetsChanged => "Origin changesets changed",
            NewIterationReason::BuildSetGraphChanged { .. } => "Build set graph changed",
            NewIterationReason::CreatedByUser => "Manually created by user",
        }
    }
}

pub enum NewBuildIterationResult {
    NoNewIterationNeeded,
    NewIterationNeeded {
        packages_to_build: HashMap<ConcreteArchitecture, BuildSetGraph>,
        reason: NewIterationReason,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IterationDiff {
    new_architectures: HashSet<ConcreteArchitecture>,
    removed_architectures: HashSet<ConcreteArchitecture>,
    changed_architectures: HashMap<ConcreteArchitecture, build_set_graph::Diff>,
}

impl IterationDiff {
    fn new(
        old_packages_to_be_built: &HashMap<ConcreteArchitecture, BuildSetGraph>,
        new_packages_to_be_built: &HashMap<ConcreteArchitecture, BuildSetGraph>,
    ) -> IterationDiff {
        let mut new_architectures = HashSet::new();
        let mut changed_architectures = HashMap::new();

        let removed_architectures = old_packages_to_be_built
            .iter()
            .filter(|(arch, _)| !new_packages_to_be_built.contains_key(arch))
            .map(|(arch, _)| *arch)
            .collect();

        for (arch, new_graph) in new_packages_to_be_built {
            if let Some(old_graph) = old_packages_to_be_built.get(arch) {
                // Architecture existed before, diff the old and new graphs
                let diff = diff_graphs(old_graph, new_graph);
                changed_architectures.insert(*arch, diff);
            } else {
                // Architecture didn't exist before, add it to the new architectures
                new_architectures.insert(*arch);
            }
        }

        IterationDiff {
            new_architectures,
            removed_architectures,
            changed_architectures,
        }
    }

    fn is_empty(&self) -> bool {
        self.changed_architectures
            .iter()
            .all(|(_, diff)| diff.is_empty())
            && self.new_architectures.is_empty()
            && self.removed_architectures.is_empty()
    }
}
pub async fn new_build_set_iteration_is_needed(
    namespace: &BuildNamespace,
    newest_iteration: Option<&BuildSetIteration>,
) -> Result<NewBuildIterationResult> {
    if namespace.status == BuildNamespaceStatus::Cancelled {
        return Ok(NewBuildIterationResult::NoNewIterationNeeded);
    }

    let packages_to_build = calculate_packages_to_be_built(namespace).await?;

    let previous_iteration = if let Some(it) = newest_iteration {
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

    let diff = IterationDiff::new(&previous_iteration.packages_to_be_built, &packages_to_build);
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

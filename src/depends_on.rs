use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;

use miette::miette;
use miette::Context;

use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;
use crate::gerrit::Gerrit;

/// A change that the current change depends on.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DependsOn {
    /// Change ID.
    pub id: ChangeId,
    /// Change number.
    pub number: ChangeNumber,
    /// Git commit hash.
    pub revision: String,
    #[serde(default)]
    pub is_current_patch_set: bool,
}

/// A change which depends on another change.
///
/// This allows constructing a graph of changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependsOnRelation {
    pub change: ChangeNumber,
    pub depends_on: ChangeNumber,
}

/// A graph of change dependencies.
#[derive(Debug, Default)]
pub struct DependsOnGraph {
    dependencies: BTreeMap<ChangeNumber, ChangeNumber>,
    reverse_dependencies: BTreeMap<ChangeNumber, BTreeSet<ChangeNumber>>,
}

impl DependsOnGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn traverse(gerrit: &Gerrit, root: ChangeNumber) -> miette::Result<Self> {
        let mut dependency_graph = Self::new();
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_front(root);

        while !queue.is_empty() {
            let change = queue.pop_back().expect("Length is checked");
            let dependencies = gerrit
                .dependencies(change)
                .wrap_err("Failed to get change dependencies")?
                .filter_unmerged(gerrit)?;
            tracing::debug!(?dependencies, "Found change dependencies");
            for depends_on in dependencies.depends_on_numbers() {
                dependency_graph.insert(DependsOnRelation { change, depends_on })?;
                if !seen.contains(&depends_on) {
                    seen.insert(depends_on);
                    queue.push_front(depends_on);
                }
            }
            for needed_by in dependencies.needed_by_numbers() {
                dependency_graph.insert(DependsOnRelation {
                    change: needed_by,
                    depends_on: change,
                })?;
                if !seen.contains(&needed_by) {
                    seen.insert(needed_by);
                    queue.push_front(needed_by);
                }
            }
        }

        Ok(dependency_graph)
    }

    pub fn insert(&mut self, dependency: DependsOnRelation) -> miette::Result<()> {
        match self.dependencies.entry(dependency.change) {
            Entry::Vacant(entry) => {
                entry.insert(dependency.depends_on);
            }
            Entry::Occupied(entry) => {
                if *entry.get() != dependency.depends_on {
                    return Err(miette!("Changes cannot depend on multiple changes: {} already depends on {} and cannot also depend on {}", entry.key(), entry.get(), dependency.depends_on));
                }
            }
        }

        self.reverse_dependencies
            .entry(dependency.depends_on)
            .or_default()
            .insert(dependency.change);

        Ok(())
    }

    pub fn depends_on(&mut self, change: ChangeNumber) -> Option<ChangeNumber> {
        self.dependencies.get(&change).copied()
    }

    pub fn needed_by(&mut self, change: ChangeNumber) -> &BTreeSet<ChangeNumber> {
        self.reverse_dependencies.entry(change).or_default()
    }

    /// Get the root dependency changes in the graph.
    ///
    /// These are the changes that do not depend on any other changes.
    pub fn depends_on_roots(&mut self, change: ChangeNumber) -> BTreeSet<ChangeNumber> {
        let mut roots = BTreeSet::new();

        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_front(change);

        while !queue.is_empty() {
            let change = queue.pop_back().expect("Length is checked");
            match self.depends_on(change) {
                Some(depends_on) => {
                    if !seen.contains(&depends_on) {
                        seen.insert(depends_on);
                        queue.push_front(depends_on);
                    }
                }
                None => {
                    roots.insert(change);
                }
            }
        }

        roots
    }
}

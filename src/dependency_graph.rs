use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::sync::Arc;

use miette::miette;
use owo_colors::OwoColorize;
use parking_lot::Mutex;

use crate::change_metadata::ChangeMetadata;
use crate::change_number::ChangeNumber;
use crate::dependency_graph_builder::DependencyGraphBuilder;
use crate::format_bulleted_list;
use crate::gerrit::Gerrit;
use crate::unicode_tree::Tree;

/// A change which depends on another change.
///
/// This allows constructing a graph of changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependsOnRelation {
    pub change: ChangeNumber,
    pub depends_on: ChangeNumber,
}

/// A graph of change dependencies.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct DependencyGraph {
    pub root: ChangeNumber,
    pub(crate) metadata: BTreeMap<ChangeNumber, ChangeMetadata>,
    pub(crate) dependencies: BTreeMap<ChangeNumber, ChangeNumber>,
    pub(crate) reverse_dependencies: BTreeMap<ChangeNumber, BTreeSet<ChangeNumber>>,
}

impl DependencyGraph {
    pub fn new(root: ChangeNumber) -> Self {
        Self {
            root,
            metadata: Default::default(),
            dependencies: Default::default(),
            reverse_dependencies: Default::default(),
        }
    }

    pub fn traverse(gerrit: &mut Gerrit, root: ChangeNumber) -> miette::Result<Self> {
        Ok(DependencyGraphBuilder::traverse(gerrit, root)?.build())
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
    pub fn depends_on_roots(&mut self) -> BTreeSet<ChangeNumber> {
        let mut roots = BTreeSet::new();

        let mut seen = BTreeSet::new();
        seen.insert(self.root);
        let mut queue = VecDeque::new();
        queue.push_front(self.root);

        while let Some(change) = queue.pop_back() {
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

    pub fn dependency_root(&mut self) -> miette::Result<ChangeNumber> {
        let mut roots = self.depends_on_roots();
        match roots.len() {
            1 => Ok(roots.pop_first().expect("Length is checked")),
            _ => Err(miette!(
                "Expected to find exactly one root change, but found {}:\n{}",
                roots.len(),
                format_bulleted_list(roots.iter())
            )),
        }
    }

    pub fn format_tree(
        &mut self,
        gerrit: &Gerrit,
        mut extra_label: impl FnMut(ChangeNumber) -> miette::Result<Vec<String>>,
    ) -> miette::Result<String> {
        let mut trees = BTreeMap::<ChangeNumber, Arc<Mutex<Tree>>>::new();
        let root = self.dependency_root()?;

        let mut seen = BTreeSet::new();
        seen.insert(root);
        let mut queue = VecDeque::new();
        queue.push_front(root);

        while let Some(change) = queue.pop_back() {
            let tree = Arc::clone(match trees.entry(change) {
                Entry::Vacant(entry) => {
                    let pretty = change.pretty(gerrit)?;
                    let mut label = vec![pretty];
                    label.extend(extra_label(change)?);
                    entry.insert(Arc::new(Mutex::new(Tree::leaf(label))))
                }
                Entry::Occupied(entry) => entry.into_mut(),
            });

            let needed_by = self.needed_by(change);
            for reverse_dependency in needed_by {
                let reverse_dependency_tree = Arc::clone(match trees.entry(*reverse_dependency) {
                    Entry::Vacant(entry) => {
                        let mut label = vec![reverse_dependency.pretty(gerrit)?];
                        label.extend(extra_label(*reverse_dependency)?);
                        entry.insert(Arc::new(Mutex::new(Tree::leaf(label))))
                    }
                    Entry::Occupied(entry) => entry.into_mut(),
                });
                tree.lock().children.push(reverse_dependency_tree);

                if !seen.contains(reverse_dependency) {
                    seen.insert(*reverse_dependency);
                    queue.push_front(*reverse_dependency);
                }
            }
        }

        let tree = trees.get(&root).expect("Root should have a tree").lock();

        Ok(tree.to_string())
    }
}

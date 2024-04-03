use std::collections::BTreeSet;

use crate::change_number::ChangeNumber;
use crate::depends_on::DependsOnGraph;
use crate::gerrit::Gerrit;

/// A chain of changes.
pub struct Chain {
    pub root: ChangeNumber,
    pub dependencies: DependsOnGraph,
}

impl Chain {
    pub fn new(gerrit: &Gerrit, root: ChangeNumber) -> miette::Result<Self> {
        Ok(Self {
            root,
            dependencies: DependsOnGraph::traverse(gerrit, root)?,
        })
    }

    /// Get the root dependency changes in the graph.
    ///
    /// These are the changes that do not depend on any other changes.
    pub fn depends_on_roots(&mut self) -> BTreeSet<ChangeNumber> {
        self.dependencies.depends_on_roots(self.root)
    }
}

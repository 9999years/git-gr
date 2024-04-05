use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;

use miette::Context;

use crate::change_metadata::ChangeMetadata;
use crate::change_number::ChangeNumber;
use crate::dependency_graph::DependencyGraph;
use crate::dependency_graph::DependsOnRelation;
use crate::gerrit::Gerrit;
use crate::query_result::ChangeDependencies;
use crate::related_changes_info::RelatedChangesInfo;

pub struct DependencyGraphBuilder<'a> {
    inner: DependencyGraph,

    gerrit: &'a mut Gerrit,
    dependencies: BTreeMap<ChangeNumber, ChangeDependencies>,
    related: BTreeMap<ChangeNumber, RelatedChangesInfo>,
}

impl<'a> DependencyGraphBuilder<'a> {
    pub fn new(gerrit: &'a mut Gerrit, root: ChangeNumber) -> Self {
        Self {
            inner: DependencyGraph::new(root),
            gerrit,
            dependencies: Default::default(),
            related: Default::default(),
        }
    }

    pub fn build(self) -> DependencyGraph {
        self.inner
    }

    fn dependencies(&mut self, change: ChangeNumber) -> miette::Result<&ChangeDependencies> {
        match self.dependencies.entry(change) {
            Entry::Vacant(entry) => {
                let change = self
                    .gerrit
                    .dependencies(change)
                    .wrap_err("Failed to get change dependencies")?
                    .filter_unmerged(self.gerrit)?;
                self.inner
                    .metadata
                    .insert(change.change.number, ChangeMetadata::new(&change.change));
                Ok(entry.insert(change))
            }
            Entry::Occupied(entry) => Ok(entry.into_mut()),
        }
    }

    fn related(&mut self, change: ChangeNumber) -> miette::Result<&RelatedChangesInfo> {
        match self.related.entry(change) {
            Entry::Vacant(entry) => {
                let change = self
                    .gerrit
                    .related_changes(change, None)
                    .wrap_err("Failed to get related changes")?;
                Ok(entry.insert(change))
            }
            Entry::Occupied(entry) => Ok(entry.into_mut()),
        }
    }

    fn indirect_reverse_dependencies(
        &mut self,
        change: ChangeNumber,
    ) -> miette::Result<BTreeSet<ChangeNumber>> {
        // If a change B depends on a change A, and A has a commit that B doesn't, in the web UI
        // you see this. On the page for A:
        //
        //     Relation chain
        //        B (Indirect relation)
        //     -> A
        //
        // On the page for B:
        //
        //     Relation chain:
        //     -> B
        //        A (Not current)
        //
        // In the API, you get this.
        // `git gr cli -- query A --dependencies`
        //     (nothing)
        //
        // `git gr api -- "/changes/PROJECT~A/revisions/current/related"`
        //     (A, B)
        //
        // `git gr cli -- query B --dependencies`
        //     dependsOn: A
        //
        // `git gr api -- "/changes/PROJECT~B/revisions/current/related"`
        //     (A, B)
        //
        // Note that `--dependencies` returns valid data for _child_ dependencies, even if they're
        // out of date...?
        //
        // Therefore, we can list out-of-date dependencies with the following logic:
        //
        //     if related(A) includes B
        //     and B depends on A
        //     then B is out of date with A
        let related_changes = self.related(change)?.change_numbers();

        let mut indirect = BTreeSet::new();
        for related in related_changes {
            if self
                .dependencies(related)?
                .depends_on_numbers()
                .contains(&change)
            {
                indirect.insert(related);
            }
        }
        Ok(indirect)
    }

    pub fn traverse(gerrit: &'a mut Gerrit, root: ChangeNumber) -> miette::Result<Self> {
        let mut builder = Self::new(gerrit, root);
        let mut seen = BTreeSet::new();
        seen.insert(root);
        let mut queue = VecDeque::new();
        queue.push_front(root);

        while let Some(change) = queue.pop_back() {
            let needed_by_indirect_numbers = builder.indirect_reverse_dependencies(change)?;
            let dependencies = builder.dependencies(change)?;
            let depends_on_numbers = dependencies.depends_on_numbers();
            let needed_by_numbers = dependencies.needed_by_numbers();
            let needed_by_numbers = needed_by_numbers.union(&needed_by_indirect_numbers);

            tracing::debug!(?dependencies, "Found change dependencies");
            for depends_on in depends_on_numbers {
                builder
                    .inner
                    .insert(DependsOnRelation { change, depends_on })?;
                if !seen.contains(&depends_on) {
                    seen.insert(depends_on);
                    queue.push_front(depends_on);
                }
            }
            for needed_by in needed_by_numbers {
                builder.inner.insert(DependsOnRelation {
                    change: *needed_by,
                    depends_on: change,
                })?;
                if !seen.contains(needed_by) {
                    seen.insert(*needed_by);
                    queue.push_front(*needed_by);
                }
            }
        }

        Ok(builder)
    }
}

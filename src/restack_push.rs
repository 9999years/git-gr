use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::io::BufReader;
use std::io::BufWriter;

use camino::Utf8PathBuf;
use fs_err::File;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;

use crate::change_number::ChangeNumber;
use crate::dependency_graph::DependencyGraph;
use crate::gerrit::GerritGitRemote;
use crate::git::Git;
use crate::restack::RefUpdate;
use crate::restack::RestackTodo;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct PushTodo {
    pub graph: DependencyGraph,
    /// Map from change numbers to updated commit hashes.
    pub refs: BTreeMap<ChangeNumber, RefUpdate>,
}

impl From<RestackTodo> for PushTodo {
    fn from(restack_todo: RestackTodo) -> Self {
        let unfiltered_refs = restack_todo.refs;
        let mut refs = BTreeMap::new();

        for (change, update) in unfiltered_refs {
            if update.has_change() {
                refs.insert(change, update);
            }
        }

        Self {
            refs,
            graph: restack_todo.graph,
        }
    }
}

impl PushTodo {
    pub fn write(&self, git: &Git) -> miette::Result<()> {
        let file = File::create(push_path(git)?).into_diagnostic()?;
        let writer = BufWriter::new(file);

        serde_json::to_writer(writer, self).into_diagnostic()?;

        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

pub fn restack_push(gerrit: &GerritGitRemote) -> miette::Result<()> {
    let mut todo = get_todo(gerrit)?;
    let git = gerrit.git();

    let root = todo.graph.dependency_root()?;
    let mut seen = BTreeSet::new();
    seen.insert(root);
    let mut queue = VecDeque::new();
    queue.push_front(root);

    tracing::info!(
        "Pushing stack:\n{}",
        todo.graph.format_tree(gerrit, |change| {
            Ok(todo
                .refs
                .get(&change)
                .into_iter()
                .map(|update| update.to_string())
                .collect())
        })?
    );

    while let Some(change) = queue.pop_back() {
        if let Some(RefUpdate { old, new }) = todo.refs.remove(&change) {
            tracing::info!(
                "Pushing change {}: {}..{}",
                change,
                old.abbrev(),
                new.abbrev(),
            );
            let change = gerrit.get_change(change)?;
            git.gerrit_push(&gerrit.remote, &new, &change.branch)?;
            todo.write(&git)?;
        }

        let needed_by = todo.graph.needed_by(change);
        for reverse_dependency in needed_by {
            if !seen.contains(reverse_dependency) {
                seen.insert(*reverse_dependency);
                queue.push_front(*reverse_dependency);
            }
        }
    }

    Ok(())
}

fn get_todo(gerrit: &GerritGitRemote) -> miette::Result<PushTodo> {
    maybe_get_todo(gerrit)?.map_err(|push_path| {
        miette!("Push todo path `{push_path}` does not exist; did you run `git-gr restack`?")
    })
}

pub fn maybe_get_todo(gerrit: &GerritGitRemote) -> miette::Result<Result<PushTodo, Utf8PathBuf>> {
    let push_path = push_path(&gerrit.git())?;

    if push_path.exists() {
        serde_json::from_reader(BufReader::new(File::open(&push_path).into_diagnostic()?))
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read push todo from `{push_path}`"))
            .map(Ok)
    } else {
        Ok(Err(push_path))
    }
}

fn push_path(git: &Git) -> miette::Result<Utf8PathBuf> {
    git.get_git_dir()
        .map(|git_dir| git_dir.join("git-gr-push-todo.json"))
}

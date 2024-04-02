use std::collections::BTreeMap;
use std::io::BufReader;
use std::io::BufWriter;

use camino::Utf8PathBuf;
use fs_err::File;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;

use crate::change_number::ChangeNumber;
use crate::gerrit::GerritGitRemote;
use crate::git::Git;
use crate::restack::RefUpdate;
use crate::restack::RestackTodo;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
pub struct PushTodo {
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

        Self { refs }
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

    while !todo.refs.is_empty() {
        let (change, RefUpdate { old, new }) = todo.refs.pop_first().expect("Length is checked");
        tracing::info!(
            "Pushing change {}: {}..{}",
            change,
            // TODO: Git hash type, short ref method.
            &old[..8],
            &new[..8],
        );
        let change = gerrit.get_change(change)?;
        git.gerrit_push(&gerrit.remote, &new, &change.branch)?;
        todo.write(&git)?;
    }

    Ok(())
}

fn get_todo(gerrit: &GerritGitRemote) -> miette::Result<PushTodo> {
    let push_path = push_path(&gerrit.git())?;

    if push_path.exists() {
        serde_json::from_reader(BufReader::new(File::open(&push_path).into_diagnostic()?))
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read push todo from `{push_path}`"))
    } else {
        Err(miette!(
            "Push todo path `{push_path}` does not exist; did you run `gayrat restack`?"
        ))
    }
}

fn push_path(git: &Git) -> miette::Result<Utf8PathBuf> {
    git.get_git_dir()
        .map(|git_dir| git_dir.join("gayrat-push-todo.json"))
}
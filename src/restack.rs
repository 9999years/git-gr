use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::fmt::Display;
use std::io::BufReader;
use std::io::BufWriter;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use fs_err as fs;
use fs_err::File;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;

use crate::change_number::ChangeNumber;
use crate::commit_hash::CommitHash;
use crate::dependency_graph::DependencyGraph;
use crate::gerrit::GerritGitRemote;
use crate::git::Git;
use crate::restack_push::PushTodo;

const CONTINUE_MESSAGE: &str = "Fix conflicts and then use `git-gr restack continue` to keep going. Alternatively, use `git-gr restack abort` to quit the restack.";

/// TODO: Add versioning?
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct RestackTodo {
    pub graph: DependencyGraph,
    /// Restack steps left to perform.
    steps: VecDeque<Step>,
    /// Map from change numbers to updated commit hashes.
    pub refs: BTreeMap<ChangeNumber, RefUpdate>,
    /// Restack step in progress, if any.
    in_progress: Option<Step>,
}

impl RestackTodo {
    pub fn write(&self, git: &Git) -> miette::Result<()> {
        let file = File::create(todo_path(git)?).into_diagnostic()?;
        let writer = BufWriter::new(file);

        serde_json::to_writer(writer, self).into_diagnostic()?;

        Ok(())
    }

    fn perform_step(
        &mut self,
        step: &Step,
        gerrit: &GerritGitRemote,
        fetched: &mut bool,
    ) -> miette::Result<()> {
        let git = gerrit.git();

        match &step.onto {
            RestackOnto::Branch { remote, branch } => {
                // Change is root, cherry-pick on target branch.
                if !*fetched {
                    git.fetch(remote)?;
                    *fetched = true;
                }
                let parent = format!("{}/{}", remote, branch);
                git.checkout_quiet(&parent)?;
                git.detach_head()?;
                let old_head = gerrit.fetch_cl(gerrit.get_change(step.change)?.patchset())?;
                let change_display = step.change.pretty(gerrit)?;
                tracing::info!("Restacking change {} on {}", change_display, branch);
                git.cherry_pick(&old_head)?;
                self.refs.insert(
                    step.change,
                    RefUpdate {
                        old: old_head,
                        new: git.get_head()?,
                    },
                );
            }
            RestackOnto::Change(parent) => {
                let change_display = step.change.pretty(gerrit)?;
                // Change is not root, cherry-pick on parent.
                let parent_ref = match self.refs.get(parent) {
                    Some(update) => {
                        tracing::debug!("Updated ref for {parent}: {update}");
                        update.new.to_owned()
                    }
                    None => {
                        let parent_ref = gerrit.fetch_cl(gerrit.get_change(*parent)?.patchset())?;
                        tracing::debug!("Fetched ref for {parent}: {}", &parent_ref[..8]);
                        parent_ref
                    }
                };
                let parent_display = parent.pretty(gerrit)?;
                git.checkout(&parent_ref)?;
                let old_head = gerrit.fetch_cl(gerrit.get_change(step.change)?.patchset())?;
                tracing::info!("Restacking change {} on {}", change_display, parent_display);
                git.cherry_pick(&old_head)?;
                self.refs.insert(
                    step.change,
                    RefUpdate {
                        old: old_head,
                        new: git.get_head()?,
                    },
                );
            }
        }

        Ok(())
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
struct Step {
    change: ChangeNumber,
    onto: RestackOnto,
}

impl Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} onto {}", self.change, self.onto)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
enum RestackOnto {
    Branch { remote: String, branch: String },
    Change(ChangeNumber),
}

impl Display for RestackOnto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestackOnto::Branch { branch, .. } => branch.fmt(f),
            RestackOnto::Change(change) => change.fmt(f),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct RefUpdate {
    pub old: CommitHash,
    pub new: CommitHash,
}

impl RefUpdate {
    pub fn has_change(&self) -> bool {
        self.old != self.new
    }
}

impl Display for RefUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", &self.old[..8], &self.new[..8],)
    }
}

pub fn restack(gerrit: &mut GerritGitRemote, branch: &str) -> miette::Result<()> {
    let git = gerrit.git();
    let mut fetched = false;
    let mut todo = get_or_create_todo(gerrit, branch)?;

    match &todo.in_progress {
        Some(step) => {
            tracing::info!("Continuing to restack {step}");
            let old_head = git.rev_parse("CHERRY_PICK_HEAD")?;
            match git
                .command()
                .args(["cherry-pick", "--continue"])
                .status_checked()
                .map(|_| ())
                .into_diagnostic()
                .wrap_err(CONTINUE_MESSAGE)
            {
                Ok(()) => {
                    todo.refs.insert(
                        step.change,
                        RefUpdate {
                            old: old_head,
                            new: git.get_head()?,
                        },
                    );
                    todo.write(&git)?;
                }
                error @ Err(_) => {
                    return error;
                }
            }
        }
        None => {}
    }

    if todo.refs.is_empty() {
        tracing::info!(
            "Restacking changes:\n{}",
            todo.graph.format_tree(gerrit, |_| Ok(Vec::new()))?
        );
    } else {
        tracing::info!(
            "Continuing to restack changes:\n{}",
            todo.graph.format_tree(gerrit, |change| {
                Ok(todo
                    .refs
                    .get(&change)
                    .into_iter()
                    .map(|update| update.to_string())
                    .collect())
            })?
        );
    }

    while let Some(step) = todo.steps.pop_front() {
        let step_result = todo
            .perform_step(&step, gerrit, &mut fetched)
            .wrap_err_with(|| format!("Failed to restack {step}"));
        match step_result {
            Ok(()) => {
                todo.write(&git)?;
            }
            error @ Err(_) => {
                todo.in_progress = Some(step);
                todo.write(&git)?;
                return error.wrap_err(CONTINUE_MESSAGE);
            }
        }
    }

    fs::remove_file(todo_path(&git)?).into_diagnostic()?;

    let mut todo = PushTodo::from(todo);
    if todo.is_empty() {
        tracing::info!("Restack completed; no changes");
    } else {
        todo.write(&git)?;
        tracing::info!(
            "Restacked changes:\n{}",
            todo.graph.format_tree(gerrit, |change| {
                Ok(todo
                    .refs
                    .get(&change)
                    .into_iter()
                    .map(|update| update.to_string())
                    .collect())
            })?
        );
        tracing::info!("Restack completed but changes have not been pushed; run `git-gr restack push` to sync changes with the remote.");
    }

    Ok(())
}

pub fn restack_abort(git: &Git) -> miette::Result<()> {
    let todo_path = todo_path(git)?;
    if todo_path.exists() {
        fs::remove_file(todo_path).into_diagnostic()?;
    }
    git.command()
        .args(["cherry-pick", "--abort"])
        .status_checked()
        .into_diagnostic()?;
    Ok(())
}

fn todo_path(git: &Git) -> miette::Result<Utf8PathBuf> {
    git.get_git_dir()
        .map(|git_dir| git_dir.join("git-gr-restack-todo.json"))
}

fn get_or_create_todo(gerrit: &mut GerritGitRemote, branch: &str) -> miette::Result<RestackTodo> {
    get_todo(gerrit)?
        .map(Ok)
        .unwrap_or_else(|| create_todo(gerrit, branch))
}

pub fn get_todo(gerrit: &GerritGitRemote) -> miette::Result<Option<RestackTodo>> {
    let todo_path = todo_path(&gerrit.git())?;

    if todo_path.exists() {
        serde_json::from_reader(BufReader::new(File::open(&todo_path).into_diagnostic()?))
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read restack todo from `{todo_path}`; remove it to abort the restack attempt"))
            .map(Some)
    } else {
        Ok(None)
    }
}

pub fn create_todo(gerrit: &mut GerritGitRemote, branch: &str) -> miette::Result<RestackTodo> {
    let git = gerrit.git();
    let todo_path = todo_path(&git)?;
    if todo_path.exists() {
        return Err(miette!("Restack todo already exists at `{todo_path}`"));
    }

    let change_id = git
        .change_id(branch)
        .wrap_err("Failed to get Change-Id for HEAD")?;
    let change = gerrit.get_change(change_id)?;
    let mut todo = RestackTodo {
        graph: gerrit.dependency_graph(change.number)?,
        steps: Default::default(),
        refs: Default::default(),
        in_progress: Default::default(),
    };

    let roots = todo.graph.depends_on_roots();
    for root in &roots {
        let mut seen = BTreeSet::new();
        seen.insert(*root);
        let mut queue = VecDeque::new();
        queue.push_front(*root);

        while let Some(change) = queue.pop_back() {
            if roots.contains(&change) {
                // Change is root, cherry-pick on target branch.
                let change = gerrit.get_change(change)?;
                let step = Step {
                    change: change.number,
                    onto: RestackOnto::Branch {
                        remote: gerrit.remote.clone(),
                        branch: change.branch,
                    },
                };
                tracing::debug!(%step, "Discovered restack step");
                todo.steps.push_back(step);
            } else {
                // Change is not root, cherry-pick on parent.
                let parent = todo
                    .graph
                    .depends_on(change)
                    .ok_or_else(|| miette!("Change does not have parent: {change}"))?;

                let step = Step {
                    change,
                    onto: RestackOnto::Change(parent),
                };
                tracing::debug!(%step, "Discovered restack step");
                todo.steps.push_back(step);
            }

            let reverse_dependencies = todo.graph.needed_by(change);

            for needed_by in reverse_dependencies {
                if !seen.contains(needed_by) {
                    seen.insert(*needed_by);
                    queue.push_front(*needed_by);
                }
            }
        }
    }

    Ok(todo)
}

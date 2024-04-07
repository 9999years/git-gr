use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::fmt::Display;
use std::io::BufReader;
use std::io::BufWriter;
use std::ops::Deref;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use fs_err as fs;
use fs_err::File;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;

use crate::change_number::ChangeNumber;
use crate::change_status::ChangeStatus;
use crate::cli::RestackContinue;
use crate::commit_hash::CommitHash;
use crate::dependency_graph::DependencyGraph;
use crate::gerrit::GerritGitRemote;
use crate::git::Git;
use crate::restack_push::PushTodo;

const CONTINUE_MESSAGE: &str = "Fix conflicts and then use `git-gr restack continue` to keep going. Alternatively, use `git-gr restack abort` to quit the restack.";

/// TODO: Add versioning?
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct RestackTodo {
    before: RepositoryState,
    pub graph: DependencyGraph,
    /// Restack steps left to perform.
    steps: VecDeque<Step>,
    /// Map from change numbers to updated commit hashes.
    pub refs: BTreeMap<ChangeNumber, RefUpdate>,
    /// Restack step in progress, if any.
    in_progress: Option<InProgress>,
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
        gerrit: &mut GerritGitRemote,
        fetched: &mut bool,
    ) -> miette::Result<()> {
        let git = gerrit.git();

        match &step.onto {
            RestackOnto::Branch { remote, branch } => {
                // Change is root, rebase on target branch.
                if !*fetched {
                    git.fetch(remote)?;
                    *fetched = true;
                }

                let old_head = gerrit.fetch_cl(gerrit.get_change(step.change)?.patchset())?;
                let change_display = step.change.pretty(gerrit)?;
                tracing::info!("Restacking change {} on {}", change_display, branch);

                let parent = format!("{}/{}", remote, branch);
                git.detach_head()?;
                gerrit.rebase_interactive(&parent)?;
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
                // Change is not root, rebase on parent.
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
                let old_head = gerrit.fetch_cl(gerrit.get_change(step.change)?.patchset())?;

                tracing::info!("Restacking change {} on {}", change_display, parent_display);
                git.detach_head()?;
                gerrit.rebase_interactive(&parent_ref)?;
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
pub struct RepositoryState {
    change: Option<ChangeNumber>,
    commit: CommitHash,
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

pub fn restack(
    gerrit: &mut GerritGitRemote,
    branch: &str,
    options: Option<RestackContinue>,
) -> miette::Result<()> {
    let git = gerrit.git();
    let mut fetched = false;
    let mut todo = get_or_create_todo(gerrit, branch)?;

    if let Some(step) = todo.in_progress.take() {
        if options
            .as_ref()
            .map(|options| options.restart_in_progress)
            .unwrap_or(false)
        {
            tracing::info!("Retrying restacking {step}");
            todo.steps.push_front(step.inner);
        } else if let Some(commit) =
            options.and_then(|mut options| options.in_progress_commit.take())
        {
            tracing::info!(
                "Using `--in-progress-commit`; restacking {step} produced commit {commit}"
            );
            todo.refs.insert(
                step.change,
                RefUpdate {
                    old: step.old_head,
                    new: commit,
                },
            );
            todo.write(&git)?;
        } else if git.rebase_in_progress()? {
            tracing::info!("Continuing to restack {step}");
            match git
                .command()
                .args(["rebase", "--continue"])
                .status_checked()
                .map(|_| ())
                .into_diagnostic()
                .wrap_err(CONTINUE_MESSAGE)
            {
                Ok(()) => {
                    todo.refs.insert(
                        step.change,
                        RefUpdate {
                            old: step.old_head,
                            new: git.get_head()?,
                        },
                    );
                    todo.write(&git)?;
                }
                error @ Err(_) => {
                    return error;
                }
            }
        } else {
            let head = git.get_head()?;
            let change_id = git.change_id(&head)?;
            let expect_change = gerrit.get_change(step.change)?;

            tracing::warn!(
                "Please use `git gr restack continue` instead of `git rebase --continue`"
            );
            if change_id == expect_change.id {
                // OK, the user just did `git rebase --continue` on their own.
                todo.refs.insert(
                    step.change,
                    RefUpdate {
                        old: step.old_head,
                        new: head,
                    },
                );
                todo.write(&git)?;
            } else {
                // The user did `git rebase --continue` on their own and then did
                // something else...
                return Err(miette!(
                    "Cannot find commit for change {}; use `git gr restack continue --in-progress-commit` or `--restart-in-progress` to continue",
                    expect_change.number.pretty(gerrit)?
                ));
            }
        }
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
        let old_head = gerrit.fetch_cl(gerrit.get_change(step.change)?.patchset())?;
        let in_progress = InProgress {
            inner: step,
            old_head,
        };

        let step_result = todo
            .perform_step(&in_progress, gerrit, &mut fetched)
            .wrap_err_with(|| format!("Failed to restack {}", in_progress));

        match step_result {
            Ok(()) => {
                todo.write(&git)?;
            }
            error @ Err(_) => {
                todo.in_progress = Some(in_progress);
                todo.write(&git)?;
                return error.wrap_err(CONTINUE_MESSAGE);
            }
        }
    }

    fs::remove_file(todo_path(&git)?).into_diagnostic()?;

    let restore = todo.before.clone();

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

    let restore_commit = match restore.change {
        Some(restore_change) => todo
            .refs
            .get(&restore_change)
            .map(|update| &update.new)
            .unwrap_or(&restore.commit),
        None => &restore.commit,
    };

    git.checkout(restore_commit)?;

    Ok(())
}

pub fn format_git_rebase_todo(gerrit: &mut GerritGitRemote) -> miette::Result<String> {
    let todo = get_todo(gerrit)?.ok_or_else(|| miette!("No restack in progress"))?;

    match todo.steps.front() {
        Some(step) => {
            let change = gerrit.get_change(step.change)?;
            let commit = gerrit.fetch_cl(change.patchset())?;
            Ok(format!(
                "pick {} {}\n",
                commit,
                change.subject.as_deref().unwrap_or("")
            ))
        }
        None => Err(miette!("Restack is already complete")),
    }
}

pub fn restack_abort(git: &Git) -> miette::Result<()> {
    let todo_path = todo_path(git)?;
    if todo_path.exists() {
        fs::remove_file(todo_path).into_diagnostic()?;
    }
    if git.rebase_in_progress()? {
        git.command()
            .args(["rebase", "--abort"])
            .status_checked()
            .into_diagnostic()?;
    }
    Ok(())
}

fn todo_path(git: &Git) -> miette::Result<Utf8PathBuf> {
    git.get_git_dir()
        .map(|git_dir| git_dir.join("git-gr-restack-todo.json"))
}

fn get_or_create_todo(gerrit: &mut GerritGitRemote, branch: &str) -> miette::Result<RestackTodo> {
    match get_todo(gerrit)? {
        Some(todo) => Ok(todo),
        None => {
            let todo = create_todo(gerrit, branch)?;
            todo.write(&gerrit.git())?;
            Ok(todo)
        }
    }
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

    let head = git.rev_parse("HEAD")?;
    let head_change = match git
        .change_id(&head)
        .and_then(|change_id| gerrit.get_change(change_id))
    {
        Ok(change) => Some(change.number),
        Err(error) => {
            tracing::debug!("Failed to get HEAD change ID: {error}");
            None
        }
    };

    let change_id = git.change_id(branch)?;
    let change = gerrit.get_change(change_id)?;
    let mut todo = RestackTodo {
        before: RepositoryState {
            change: head_change,
            commit: head,
        },
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
            let change = gerrit.get_change(change)?;

            match change.status {
                ChangeStatus::New => {
                    // Carry on.
                }
                ChangeStatus::Merged | ChangeStatus::Abandoned => {
                    tracing::debug!("Skipping merged/abandoned change {}", change.number);
                    continue;
                }
            }

            if roots.contains(&change.number) {
                // Change is root, cherry-pick on target branch.
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
                    .depends_on(change.number)
                    .ok_or_else(|| miette!("Change does not have parent: {}", change.number))?;

                let step = Step {
                    change: change.number,
                    onto: RestackOnto::Change(parent),
                };
                tracing::debug!(%step, "Discovered restack step");
                todo.steps.push_back(step);
            }

            let reverse_dependencies = todo.graph.needed_by(change.number);

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

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
struct InProgress {
    /// The step in progress.
    inner: Step,
    /// The HEAD commit of the change before restacking.
    old_head: CommitHash,
}

impl Deref for InProgress {
    type Target = Step;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Display for InProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

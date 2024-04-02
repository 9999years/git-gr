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
use crate::format_bulleted_list;
use crate::gerrit::GerritGitRemote;
use crate::git::Git;
use crate::restack_push::PushTodo;

const CONTINUE_MESSAGE: &str = "Fix conflicts and then use `gayrat restack continue` to keep going. Alternatively, use `gayrat restack abort` to quit the restack.";

/// TODO: Add versioning?
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
pub struct RestackTodo {
    /// Rebase steps left to perform.
    steps: VecDeque<Rebase>,
    /// Map from change numbers to updated commit hashes.
    pub refs: BTreeMap<ChangeNumber, RefUpdate>,
    /// Rebase step in progress, if any.
    in_progress: Option<Rebase>,
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
        step: &Rebase,
        gerrit: &GerritGitRemote,
        fetched: &mut bool,
    ) -> miette::Result<()> {
        let git = gerrit.git();

        match &step.onto {
            RebaseOnto::Branch { remote, branch } => {
                if !*fetched {
                    git.fetch(remote)?;
                    *fetched = true;
                }
                gerrit.checkout_cl_quiet(step.change)?;
                let old_head = git.get_head()?;
                // Change is root, rebase on target branch.
                let change_display = step.change.pretty(gerrit)?;
                tracing::info!("Restacking change {} on {}", change_display, branch);
                git.rebase(&format!("{}/{}", remote, branch))?;
                self.refs.insert(
                    step.change,
                    RefUpdate {
                        old: old_head,
                        new: git.get_head()?,
                    },
                );
            }
            RebaseOnto::Change(parent) => {
                let change_display = step.change.pretty(gerrit)?;
                // Change is not root, rebase on parent.
                let parent_ref = match self.refs.get(parent) {
                    Some(update) => update.new.to_owned(),
                    None => gerrit.fetch_cl_quiet(*parent)?,
                };
                let parent_display = parent.pretty(gerrit)?;
                gerrit.checkout_cl_quiet(step.change)?;
                let old_head = git.get_head()?;
                tracing::info!("Restacking change {} on {}", change_display, parent_display);
                git.rebase(&parent_ref)?;
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
struct Rebase {
    change: ChangeNumber,
    onto: RebaseOnto,
}

impl Display for Rebase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} onto {}", self.change, self.onto)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
enum RebaseOnto {
    Branch { remote: String, branch: String },
    Change(ChangeNumber),
}

impl Display for RebaseOnto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RebaseOnto::Branch { branch, .. } => branch.fmt(f),
            RebaseOnto::Change(change) => change.fmt(f),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
pub struct RefUpdate {
    pub old: String,
    pub new: String,
}

impl RefUpdate {
    pub fn has_change(&self) -> bool {
        self.old != self.new
    }
}

pub fn restack(gerrit: &GerritGitRemote) -> miette::Result<()> {
    let git = gerrit.git();
    let mut fetched = false;
    let mut todo = get_or_create_todo(gerrit)?;

    match &todo.in_progress {
        Some(step) => {
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
                    todo.write(&git)?;
                }
                error @ Err(_) => {
                    return error;
                }
            }
        }
        None => {}
    }

    while !todo.steps.is_empty() {
        let step = todo.steps.pop_front().expect("Length is checked");

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

    let todo = PushTodo::from(todo);
    todo.write(&git)?;
    tracing::info!(
        "Restacked changes:\n{}",
        format_bulleted_list(todo.refs.iter().map(|(change, RefUpdate { old, new })| {
            format!("{}: {}..{}", change, &old[..8], &new[..8],)
        }))
    );
    tracing::info!("Restack completed but changes have not been pushed; run `gayrat restack push` to sync changes with the remote.");

    Ok(())
}

pub fn restack_abort(git: &Git) -> miette::Result<()> {
    let todo_path = todo_path(git)?;
    if todo_path.exists() {
        fs::remove_file(todo_path).into_diagnostic()?;
    }
    git.command()
        .args(["rebase", "--abort"])
        .status_checked()
        .into_diagnostic()?;
    Ok(())
}

fn todo_path(git: &Git) -> miette::Result<Utf8PathBuf> {
    git.get_git_dir()
        .map(|git_dir| git_dir.join("gayrat-restack-todo.json"))
}

fn get_or_create_todo(gerrit: &GerritGitRemote) -> miette::Result<RestackTodo> {
    let todo_path = todo_path(&gerrit.git())?;

    if todo_path.exists() {
        serde_json::from_reader(BufReader::new(File::open(&todo_path).into_diagnostic()?))
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read restack todo from `{todo_path}`; remove it to abort the restack attempt"))
    } else {
        create_todo(gerrit)
    }
}

fn create_todo(gerrit: &GerritGitRemote) -> miette::Result<RestackTodo> {
    let git = gerrit.git();
    let change_id = git
        .change_id("HEAD")
        .wrap_err("Failed to get Change-Id for HEAD")?;
    let mut chain = gerrit.dependency_graph(&change_id)?;
    let mut todo = RestackTodo::default();

    let roots = chain.depends_on_roots();
    for root in &roots {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_front(*root);

        while !queue.is_empty() {
            let change = queue.pop_back().expect("Length is checked");

            if roots.contains(&change) {
                // Change is root, rebase on target branch.
                let change = gerrit.get_current_patch_set(change)?;
                let step = Rebase {
                    change: change.change.number,
                    onto: RebaseOnto::Branch {
                        remote: gerrit.remote.clone(),
                        branch: change.change.branch,
                    },
                };
                tracing::debug!(%step, "Discovered rebase step");
                todo.steps.push_back(step);
            } else {
                // Change is not root, rebase on parent.
                let parent = chain.dependencies.depends_on(change).ok_or_else(|| {
                    miette!("Change does not have parent to rebase onto: {change}")
                })?;

                let step = Rebase {
                    change,
                    onto: RebaseOnto::Change(parent),
                };
                tracing::debug!(%step, "Discovered rebase step");
                todo.steps.push_back(step);
            }

            let reverse_dependencies = chain.dependencies.needed_by(change);

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

use std::collections::BTreeSet;
use std::collections::VecDeque;

use miette::miette;

mod approval;
mod author;
mod chain;
mod change_id;
mod change_number;
mod cli;
mod current_patch_set;
mod depends_on;
mod format_bulleted_list;
mod gerrit;
mod gerrit_query;
mod git;
mod install_tracing;
mod needed_by;
mod query_result;
mod tmpdir;

use clap::Parser;
use cli::Opts;
use command_error::CommandExt;
use format_bulleted_list::format_bulleted_list;
use git::Git;
use install_tracing::install_tracing;
use miette::Context;
use miette::IntoDiagnostic;

fn main() -> miette::Result<()> {
    let opts = Opts::parse();
    install_tracing(&opts.log)?;

    match opts.command {
        cli::Command::Push { branch, target } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            let target = match target {
                Some(target) => target,
                None => git.default_branch(&gerrit.remote)?,
            };
            let branch = match branch {
                Some(branch) => branch,
                None => "HEAD".to_owned(),
            };
            git.gerrit_push(&gerrit.remote, &branch, &target)?;
        }
        cli::Command::Checkout { number } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            gerrit.checkout_cl(&number.to_string())?;
        }
        cli::Command::Up => {
            let git = Git::new();
            let change_id = git
                .change_id("HEAD")
                .wrap_err("Failed to get Change-Id for HEAD")?;
            let gerrit = git.gerrit(None)?;
            let dependencies = gerrit
                .dependencies(&change_id)
                .wrap_err("Failed to get change dependencies")?;
            let mut needed_by = dependencies.needed_by_numbers();
            let needed_by = match needed_by.len() {
                0 => {
                    return Err(miette!(
                        "Change {} isn't needed by any changes",
                        dependencies.change.number
                    ));
                }
                1 => needed_by.pop_first().expect("Length was checked"),
                _ => {
                    return Err(miette!(
                        "Change {} is needed by multiple changes, and selecting between them is unimplemented:\n{}",
                        dependencies.change.number,
                        format_bulleted_list(needed_by)
                    ));
                }
            };
            gerrit.checkout_cl(&needed_by.to_string())?;
        }
        cli::Command::Down => {
            let git = Git::new();
            let change_id = git
                .change_id("HEAD")
                .wrap_err("Failed to get Change-Id for HEAD")?;
            let gerrit = git.gerrit(None)?;
            let dependencies = gerrit
                .dependencies(&change_id)
                .wrap_err("Failed to get change dependencies")?;
            let mut depends_on = dependencies.depends_on_numbers();
            let depends_on = match depends_on.len() {
                0 => {
                    return Err(miette!(
                        "Change {} doesn't depend on any changes",
                        dependencies.change.number
                    ));
                }
                1 => depends_on.pop_first().expect("Length was checked"),
                _ => {
                    return Err(miette!(
                        "Change {} depends on multiple changes, and selecting between them is unimplemented:\n{}",
                        dependencies.change.number,
                        format_bulleted_list(&depends_on)
                    ));
                }
            };
            gerrit.checkout_cl(&depends_on.to_string())?;
        }
        cli::Command::Cli { args } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            gerrit.command(args).status_checked().into_diagnostic()?;
        }
        cli::Command::Restack => {
            let git = Git::new();
            let change_id = git
                .change_id("HEAD")
                .wrap_err("Failed to get Change-Id for HEAD")?;
            let gerrit = git.gerrit(None)?;
            let mut chain = gerrit.dependency_graph(&change_id)?;
            // TODO: Serialize graph to disk so we can continue when there's merge
            // conflicts.
            let roots = chain.depends_on_roots();
            for root in &roots {
                let mut seen = BTreeSet::new();
                let mut queue = VecDeque::new();
                queue.push_front(*root);

                while !queue.is_empty() {
                    let change = queue.pop_back().expect("Length is checked");

                    if roots.contains(&change) {
                        // Change is root, rebase on target branch.
                        let change = gerrit.get_current_patch_set(&change.to_string())?;
                        gerrit.checkout_cl(&change.change.number.to_string())?;
                        git.rebase(&format!("{}/{}", gerrit.remote, change.change.branch))?;
                    } else {
                        // Change is not root, rebase on parent.
                        let parent = chain.dependencies.depends_on(change).ok_or_else(|| {
                            miette!("Change does not have parent to rebase onto: {change}")
                        })?;
                        let parent_ref = gerrit.fetch_cl(&parent.to_string())?;
                        gerrit.checkout_cl(&change.to_string())?;
                        git.rebase(&parent_ref)?;
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
        }
    }

    Ok(())
}

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
mod git;
mod install_tracing;
mod needed_by;
mod query;
mod query_result;
mod restack;
mod restack_push;
mod tmpdir;

use calm_io::stdoutln;
use clap::Parser;
use cli::Opts;
use command_error::CommandExt;
use format_bulleted_list::format_bulleted_list;
use git::Git;
use install_tracing::install_tracing;
use miette::Context;
use miette::IntoDiagnostic;
use restack::create_todo;

fn main() -> miette::Result<()> {
    let opts = Opts::parse();
    install_tracing(&opts.log)?;

    match opts.command {
        cli::Command::Push {
            branch,
            target,
            restack,
        } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            if restack {
                let branch_str = branch.as_deref().unwrap_or("HEAD");
                let todo = create_todo(&gerrit, branch_str)?;
                todo.write(&git)?;
                gerrit.push(branch.clone(), target)?;
                gerrit.restack(branch_str)?;
            } else {
                gerrit.push(branch, target)?;
            }
        }
        cli::Command::Checkout { patchset, number } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            match patchset {
                Some(patchset) => {
                    gerrit.checkout_cl_patchset(number, patchset)?;
                }
                None => {
                    gerrit.checkout_cl(number)?;
                }
            }
        }
        cli::Command::Fetch { number } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            let git_ref = gerrit.fetch_cl(number)?;
            let _ = stdoutln!("{git_ref}");
        }
        cli::Command::Up => {
            let git = Git::new();
            let change_id = git
                .change_id("HEAD")
                .wrap_err("Failed to get Change-Id for HEAD")?;
            let gerrit = git.gerrit(None)?;
            let dependencies = gerrit
                .dependencies(&change_id)
                .wrap_err("Failed to get change dependencies")?
                .filter_unmerged(&gerrit)?;
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
                        "Change {} is needed by multiple changes; use `gayrat checkout {}` to pick one:\n{}",
                        dependencies.change.number,
                        dependencies.change.number,
                        format_bulleted_list(needed_by)
                    ));
                }
            };
            gerrit.checkout_cl(needed_by)?;
        }
        cli::Command::Top => {
            let git = Git::new();
            let change_id = git
                .change_id("HEAD")
                .wrap_err("Failed to get Change-Id for HEAD")?;
            let gerrit = git.gerrit(None)?;
            let change = gerrit.get_change(&change_id)?.number;
            let mut next = change;

            loop {
                let mut needed_by = gerrit
                    .dependencies(next)
                    .wrap_err("Failed to get change dependencies")?
                    .filter_unmerged(&gerrit)?
                    .needed_by_numbers();

                next = match needed_by.len() {
                    0 => {
                        break;
                    }
                    1 => needed_by.pop_first().expect("Length was checked"),
                    _ => {
                        return Err(miette!(
                        "Change {} is needed by multiple changes; use `gayrat checkout {}` to pick one:\n{}",
                        next,
                        next,
                        format_bulleted_list(needed_by)
                    ));
                    }
                };
            }
            gerrit.checkout_cl(next)?;
        }
        cli::Command::Down => {
            let git = Git::new();
            let change_id = git
                .change_id("HEAD")
                .wrap_err("Failed to get Change-Id for HEAD")?;
            let gerrit = git.gerrit(None)?;
            let dependencies = gerrit
                .dependencies(&change_id)
                .wrap_err("Failed to get change dependencies")?
                .filter_unmerged(&gerrit)?;
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
                        "Change {} depends on multiple changes, use `gayrat checkout {}` to pick one:\n{}",
                        dependencies.change.number,
                        dependencies.change.number,
                        format_bulleted_list(&depends_on)
                    ));
                }
            };
            gerrit.checkout_cl(depends_on)?;
        }
        cli::Command::Cli { args } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            gerrit.command(args).status_checked().into_diagnostic()?;
        }
        cli::Command::Restack { command } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            match command {
                None => {
                    gerrit.restack("HEAD")?;
                }
                Some(command) => match command {
                    cli::Restack::Continue => {
                        gerrit.restack_continue()?;
                    }
                    cli::Restack::Abort => {
                        gerrit.restack_abort()?;
                    }
                    cli::Restack::Push => {
                        gerrit.restack_push()?;
                    }
                    cli::Restack::This => {
                        gerrit.restack_this()?;
                    }
                },
            }
        }
    }

    Ok(())
}

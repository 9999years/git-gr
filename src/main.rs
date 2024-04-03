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
            let gerrit = git.gerrit(None)?;
            gerrit.up()?;
        }
        cli::Command::Top => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            gerrit.top()?;
        }
        cli::Command::Down => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            gerrit.down()?;
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

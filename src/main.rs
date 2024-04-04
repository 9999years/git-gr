mod approval;
mod author;
mod change;
mod change_id;
mod change_number;
mod change_status;
mod cli;
mod commit_hash;
mod commit_info;
mod current_patch_set;
mod dependency_graph;
mod dependency_graph_builder;
mod depends_on;
mod format_bulleted_list;
mod gerrit;
mod git;
mod git_person_info;
mod install_tracing;
mod needed_by;
mod query;
mod query_result;
mod related_change_and_commit_info;
mod related_changes_info;
mod restack;
mod restack_push;
mod submit_label;
mod submit_label_status;
mod submit_records;
mod submit_status;
mod tmpdir;
mod unicode_tree;

use calm_io::stdoutln;
use clap::CommandFactory;
use clap::Parser;
use cli::Opts;
use command_error::CommandExt;
use format_bulleted_list::format_bulleted_list;
use git::Git;
use install_tracing::install_tracing;
use miette::IntoDiagnostic;
use restack::create_todo;

#[allow(unused_imports)]
use miette::Context;

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
            let mut gerrit = git.gerrit(None)?;
            if restack {
                let branch_str = branch.as_deref().unwrap_or("HEAD");
                let todo = create_todo(&mut gerrit, branch_str)?;
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
            let mut gerrit = git.gerrit(None)?;
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
        cli::Command::Completions { shell } => {
            let mut clap_command = cli::Opts::command();
            clap_complete::generate(shell, &mut clap_command, "git-gr", &mut std::io::stdout());
        }
        #[cfg(feature = "clap_mangen")]
        cli::Command::Manpages { out_dir } => {
            let clap_command = cli::Opts::command();
            clap_mangen::generate_to(clap_command, out_dir)
                .into_diagnostic()
                .wrap_err("Failed to generate man pages")?;
        }
        cli::Command::Query { query } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            gerrit.print_query(query)?;
        }
        cli::Command::Api { method, endpoint } => {
            let git = Git::new();
            let mut gerrit = git.gerrit(None)?;
            let response = gerrit.http_request(method, &endpoint)?;
            let _ = stdoutln!("{response}");
        }
        cli::Command::ShowChain { query } => {
            let git = Git::new();
            let mut gerrit = git.gerrit(None)?;
            let chain = gerrit.format_chain(query)?;
            let _ = stdoutln!("{chain}");
        }
    }

    Ok(())
}

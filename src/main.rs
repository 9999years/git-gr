mod approval;
mod author;
mod cache;
mod change;
mod change_id;
mod change_key;
mod change_number;
mod change_status;
mod cli;
mod commit_hash;
mod commit_info;
mod config;
mod current_exe;
mod current_patch_set;
mod dependency_graph;
mod dependency_graph_builder;
mod depends_on;
mod endpoint;
mod format_bulleted_list;
mod gerrit;
mod gerrit_host;
mod gerrit_project;
mod git;
mod git_person_info;
mod install_tracing;
mod needed_by;
mod patchset;
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
use cli::Args;
use command_error::CommandExt;
use format_bulleted_list::format_bulleted_list;
use git::Git;
use install_tracing::install_tracing;
use miette::IntoDiagnostic;
use patchset::ChangePatchset;
use restack::create_todo;

#[allow(unused_imports)]
use miette::Context;

fn main() -> miette::Result<()> {
    let opts = Args::parse();
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
                gerrit.restack(branch_str, None)?;
            } else {
                gerrit.push(branch, target)?;
            }
        }
        cli::Command::Checkout { patchset, number } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            match patchset {
                Some(patchset) => {
                    gerrit.checkout_cl(ChangePatchset {
                        change: number,
                        patchset,
                    })?;
                }
                None => {
                    gerrit.checkout_cl(gerrit.get_change(number)?.patchset())?;
                }
            }
        }
        cli::Command::Fetch { number } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            let change = gerrit.get_change(number)?;
            let git_ref = gerrit.fetch_cl(change.patchset())?;
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
                    gerrit.restack("HEAD", None)?;
                }
                Some(command) => match command {
                    cli::Restack::Continue(restack_continue) => {
                        gerrit.restack_continue(restack_continue)?;
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
                    cli::Restack::WriteTodo { git_rebase_todo } => {
                        gerrit.restack_write_git_rebase_todo(&git_rebase_todo)?;
                    }
                },
            }
        }
        cli::Command::Completions { shell } => {
            let mut clap_command = cli::Args::command();
            clap_complete::generate(shell, &mut clap_command, "git-gr", &mut std::io::stdout());
        }
        #[cfg(feature = "clap_mangen")]
        cli::Command::Manpages { out_dir } => {
            let clap_command = cli::Args::command();
            clap_mangen::generate_to(clap_command, out_dir)
                .into_diagnostic()
                .wrap_err("Failed to generate man pages")?;
        }
        cli::Command::Query {
            query,
            mine,
            needs_review,
        } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;

            let mut query = match query {
                Some(query) => query,
                None => {
                    if mine || needs_review {
                        "".to_owned()
                    } else {
                        "status:open -is:wip".to_owned()
                    }
                }
            };

            if mine {
                query.push_str(" is:open owner:self");
            }
            if needs_review {
                if !mine {
                    query.push_str(" is:open -owner:self");
                }
                query.push_str(" -is:wip -is:reviewed");
            }
            let table = gerrit.format_query_results(query)?;

            let _ = stdoutln!("{table}");
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
        cli::Command::View { query } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            let query = match query {
                Some(query) => query,
                None => git.change_id("HEAD")?.into(),
            };
            let change = gerrit.get_change(query)?;
            let url = &change.url;
            webbrowser::open(url)
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to open browser for {url}"))?;
        }
        cli::Command::ClearCache => {
            let git = Git::new();
            let mut gerrit = git.gerrit(None)?;
            gerrit.clear_cache();
        }
    }

    Ok(())
}

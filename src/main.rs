use miette::miette;

mod approval;
mod author;
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
            gerrit.checkout_cl(number)?;
        }
        cli::Command::Up => {
            let git = Git::new();
            let change_id = git
                .change_id("HEAD")
                .wrap_err("Failed to get Change-Id for HEAD")?;
            let gerrit = git.gerrit(None)?;
            let mut dependencies = gerrit
                .dependencies(change_id)
                .wrap_err("Failed to get change dependencies")?;
            let needed_by = match dependencies.needed_by.len() {
                0 => {
                    return Err(miette!(
                        "Change {} isn't needed by any other changes",
                        dependencies.change.number
                    ));
                }
                1 => dependencies.needed_by.pop().expect("Length was checked"),
                _ => {
                    return Err(miette!(
                        "Change {} is needed by multiple changes, and selecting between them is unimplemented:\n{}",
                        dependencies.change.number,
                        format_bulleted_list(dependencies.needed_by.iter().map(|change| change.number))
                    ));
                }
            };
            gerrit.checkout_cl(needed_by.number)?;
        }
        cli::Command::Down => todo!(),
        cli::Command::Cli { args } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            gerrit.command(args).status_checked().into_diagnostic()?;
        }
    }

    Ok(())
}
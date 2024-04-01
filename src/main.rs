mod change_id;
mod change_number;
mod cli;
mod depends_on;
mod format_bulleted_list;
mod gerrit;
mod gerrit_query;
mod git;
mod install_tracing;

use clap::Parser;
use cli::Opts;
use command_error::CommandExt;
use format_bulleted_list::format_bulleted_list;
use git::Git;
use install_tracing::install_tracing;
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
        cli::Command::Checkout { number } => todo!(),
        cli::Command::Up => todo!(),
        cli::Command::Down => todo!(),
        cli::Command::Cli { args } => {
            let git = Git::new();
            let gerrit = git.gerrit(None)?;
            gerrit.command(args).status_checked().into_diagnostic()?;
        }
    }

    Ok(())
}

use clap::Parser;
use clap::Subcommand;

use crate::change_number::ChangeNumber;

/// A Gerrit CLI.
#[derive(Debug, Clone, Parser)]
#[command(version, author, about)]
#[command(max_term_width = 100, disable_help_subcommand = true)]
pub struct Opts {
    /// Log filter directives, of the form `target[span{field=value}]=level`, where all components
    /// except the level are optional.
    ///
    /// Try `debug` or `trace`.
    #[arg(long, default_value = "info", env = "GAYRAT_LOG")]
    pub log: String,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Push a branch to Gerrit.
    Push {
        /// The branch or commit to push. Defaults to `HEAD`.
        #[arg(short, long)]
        branch: Option<String>,

        /// The branch to target the CL against.
        ///
        /// Defaults to the default upstream branch.
        #[arg()]
        target: Option<String>,
    },
    /// Checkout a CL.
    Checkout {
        /// The change number to checkout.
        number: ChangeNumber,
    },
    /// Rebase each CL in a stack, ensuring it's up-to-date with its parent.
    Restack {
        /// Only restack this CL and its descendants.
        #[arg(long)]
        upstack: bool,

        /// Only restack this CL and its ancestors.
        #[arg(long)]
        downstack: bool,
    },
    /// Checkout the next CL above this one in the stack.
    Up,
    /// Checkout the next CL below this one in the stack.
    Down,
    /// Run a `gerrit` command on the remote server.
    Cli {
        /// Arguments to pass to `gerrit`.
        args: Vec<String>,
    },
}

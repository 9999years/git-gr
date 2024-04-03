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

        /// Push and then restack changes that depend on the branch.
        #[arg(long)]
        restack: bool,
    },
    /// Checkout a CL.
    Checkout {
        /// The change number to checkout.
        number: ChangeNumber,
        /// The patchset number to checkout, if any.
        ///
        /// Defaults to the latest patchset.
        #[arg(short, long)]
        patchset: Option<u32>,
    },
    /// Fetch a CL.
    Fetch {
        /// The change number to fetch.
        number: ChangeNumber,
    },
    /// Rebase each CL in a stack, ensuring it's up-to-date with its parent.
    Restack {
        #[command(subcommand)]
        command: Option<Restack>,
    },
    /// Checkout the next CL above this one in the stack.
    Up,
    /// Checkout the top-most CL in the stack.
    Top,
    /// Checkout the next CL below this one in the stack.
    Down,
    /// Run a `gerrit` command on the remote server.
    Cli {
        /// Arguments to pass to `gerrit`.
        args: Vec<String>,
    },
    /// Generate shell completions.
    Completions {
        /// Shell to generate completions for.
        shell: clap_complete::shells::Shell,
    },
    /// Generate man pages.
    #[cfg(feature = "clap_mangen")]
    Manpages {
        /// Directory to write man pages to.
        out_dir: camino::Utf8PathBuf,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum Restack {
    /// Restack only the currently checked-out CL on its immediate ancestor.
    This,
    /// Continue an in-progress restack.
    Continue,
    /// Abort an in-progress restack.
    Abort,
    /// Push changes from a completed restack.
    Push,
}

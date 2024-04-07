use camino::Utf8PathBuf;
use clap::Args;
use clap::Parser;
use clap::Subcommand;
use reqwest::Method;

use crate::change_number::ChangeNumber;
use crate::commit_hash::CommitHash;
use crate::endpoint::Endpoint;
use crate::patchset::Patchset;

/// A Gerrit CLI.
#[derive(Debug, Clone, Parser)]
#[command(version, author, about)]
#[command(max_term_width = 100, disable_help_subcommand = true)]
pub struct Opts {
    /// Log filter directives, of the form `target[span{field=value}]=level`, where all components
    /// except the level are optional.
    ///
    /// Try `debug` or `trace`.
    #[arg(long, default_value = "info", env = "GIT_GR_LOG")]
    pub log: String,

    #[command(subcommand)]
    pub command: Command,
}

#[allow(rustdoc::bare_urls)]
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
        patchset: Option<Patchset>,
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
    /// Query changes.
    Query {
        /// Show change you own.
        ///
        /// Adds `is:open owner:self` to the query.
        #[arg(long)]
        mine: bool,

        /// Show changes by others that need review.
        ///
        /// Adds `is:open -owner:self -is:wip -is:reviewed` to the query.
        #[arg(long)]
        needs_review: bool,

        /// Query to search for.
        ///
        /// Defaults to `status:open -is:wip`.
        ///
        /// See: https://gerrit.lix.systems/Documentation/user-search.html
        query: Option<String>,
    },
    /// Run a `gerrit` command on the remote server.
    Cli {
        /// Arguments to pass to `gerrit`.
        args: Vec<String>,
    },
    /// Make a request to the Gerrit REST API.
    ///
    /// See: https://gerrit-review.googlesource.com/Documentation/rest-api.html
    Api {
        #[arg(short = 'X', long, default_value_t = reqwest::Method::GET)]
        method: Method,
        endpoint: Endpoint,
    },
    /// Display a chain of related changes.
    ShowChain {
        /// A query for the change to show.
        ///
        /// Defaults to the `HEAD` commit's change.
        query: Option<String>,
    },
    /// Open a change in a web browser.
    View {
        /// The change to view.
        ///
        /// Defaults to the `HEAD` commit's change.
        query: Option<String>,
    },
    /// Clear the cache of changes and API responses.
    ClearCache,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Restack {
    /// Restack only the currently checked-out CL on its immediate ancestor.
    This,
    /// Continue an in-progress restack.
    Continue(RestackContinue),
    /// Abort an in-progress restack.
    Abort,
    /// Push changes from a completed restack.
    Push,
    /// Write `git-rebase-todo`.
    #[command(hide = true)]
    WriteTodo {
        /// `git-rebase-todo` path to write to.
        #[arg()]
        git_rebase_todo: Utf8PathBuf,
    },
}

#[derive(Debug, Clone, Args)]
pub struct RestackContinue {
    /// If you ran `git rebase --continue` on your own and then checked something else out,
    /// `git-gr` will not be able to determine the new commit hash for the in-progress restack
    /// step. Use this flag to supply it manually.
    #[arg(long)]
    pub in_progress_commit: Option<CommitHash>,

    /// If you ran `git rebase --continue` on your own and then checked something else out,
    /// `git-gr` will not be able to determine the new commit hash for the in-progress restack
    /// step. Use this flag to restart the in-progress restack step, abandoning any changes you
    /// may have made.
    #[arg(long)]
    pub restart_in_progress: bool,
}

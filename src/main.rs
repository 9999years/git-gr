use clap::Parser;

mod format_bulleted_list;
mod install_tracing;

use install_tracing::install_tracing;

/// A Gerrit CLI.
#[derive(Debug, Clone, Parser)]
#[command(version, author, about)]
#[command(max_term_width = 100, disable_help_subcommand = true)]
pub struct Opts {
    /// Log filter directives, of the form `target[span{field=value}]=level`, where all components
    /// except the level are optional.
    ///
    /// Try `debug` or `trace`.
    #[arg(long, default_value = "info", env = "GIT_UPSTREAM_LOG")]
    log: String,
}

fn main() -> miette::Result<()> {
    let opts = Opts::parse();
    install_tracing(&opts.log)?;

    Ok(())
}

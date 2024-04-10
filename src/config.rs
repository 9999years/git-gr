use crate::cli::Args;
use crate::git::Git;

pub struct Config {
    git: Git,
    args: Args,
}

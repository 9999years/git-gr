use std::process::Command;
use std::sync::OnceLock;

use camino::Utf8PathBuf;
use command_error::CommandExt;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use regex::Regex;

use crate::change_id::ChangeId;
use crate::commit_hash::CommitHash;
use crate::format_bulleted_list;
use crate::gerrit::GerritGitRemote;

/// `git` CLI wrapper.
#[derive(Debug, Default)]
pub struct Git {}

impl Git {
    pub fn new() -> Self {
        Default::default()
    }

    /// Get a `git` command.
    pub fn command(&self) -> Command {
        Command::new("git")
    }

    /// Push to a `refs/for/{branch}` ref.
    pub fn gerrit_push(&self, remote: &str, commitish: &str, target: &str) -> miette::Result<()> {
        self.command()
            .args(["push", remote, &format!("{commitish}:refs/for/{target}")])
            .status_checked()
            .map(|_| ())
            .into_diagnostic()
    }

    /// Get a list of all `git remote`s.
    pub fn remotes(&self) -> miette::Result<Vec<String>> {
        Ok(self
            .command()
            .arg("remote")
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to list Git remotes")?
            .stdout
            .lines()
            .map(|line| line.to_owned())
            .collect())
    }

    /// Get the (push) URL for the given remote.
    pub fn remote_url(&self, remote: &str) -> miette::Result<String> {
        Ok(self
            .command()
            .args(["remote", "get-url", "--push", &remote])
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to get Git remote URL")?
            .stdout
            .trim()
            .to_owned())
    }

    fn default_branch_symbolic_ref(&self, remote: &str) -> miette::Result<String> {
        let output = self
            .command()
            .args([
                "symbolic-ref",
                "--short",
                &format!("refs/remotes/{remote}/HEAD"),
            ])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        static RE: OnceLock<Regex> = OnceLock::new();
        let captures = RE
            .get_or_init(|| {
                Regex::new(
                    r"(?xm)
                    ^
                    (?P<remote>[[:word:]]+)/(?P<branch>[[:word:]]+)
                    $
                    ",
                )
                .expect("Regex parses")
            })
            .captures(&output);

        match captures {
            Some(captures) => Ok(captures["branch"].to_owned()),
            None => Err(miette!(
                "Could not parse `git symbolic-ref` output:\n{output}"
            )),
        }
    }

    fn default_branch_ls_remote(&self, remote: &str) -> miette::Result<String> {
        let output = self
            .command()
            .args(["ls-remote", "--symref", remote, "HEAD"])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        static RE: OnceLock<Regex> = OnceLock::new();
        let captures = RE
            .get_or_init(|| {
                Regex::new(
                    r"(?xm)
                    ^
                    ref: refs/heads/(?P<branch>[[:word:]]+)\tHEAD
                    $
                    ",
                )
                .expect("Regex parses")
            })
            .captures(&output);

        match captures {
            Some(captures) => Ok(captures["branch"].to_owned()),
            None => Err(miette!("Could not parse `git ls-remote` output:\n{output}")),
        }
    }

    pub fn default_branch(&self, remote: &str) -> miette::Result<String> {
        self.default_branch_symbolic_ref(remote).or_else(|err| {
            tracing::debug!("Failed to get default branch: {err}");
            self.default_branch_ls_remote(remote)
        })
    }

    pub fn commit_message(&self, commit: &str) -> miette::Result<String> {
        Ok(self
            .command()
            .args(["show", "--no-patch", "--format=%B", &commit])
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to get commit message")?
            .stdout)
    }

    pub fn change_id(&self, commit: &str) -> miette::Result<ChangeId> {
        let commit_message = self.commit_message(commit)?;

        static RE: OnceLock<Regex> = OnceLock::new();
        let captures = RE
            .get_or_init(|| {
                Regex::new(
                    r"(?xm)
                    ^
                    Change-Id:\ (?P<change_id>I[[:xdigit:]]{40})
                    $
                    ",
                )
                .expect("Regex parses")
            })
            .captures(&commit_message);

        match captures {
            Some(captures) => Ok(ChangeId::new(captures["change_id"].to_owned())),
            None => Err(miette!(
                "Could not find Change-Id in message for commit {commit}:\n{commit_message}"
            )),
        }
    }

    pub fn gerrit(&self, gerrit_remote_name: Option<&str>) -> miette::Result<GerritGitRemote> {
        let mut tried = Vec::new();

        if let Some(remote_name) = gerrit_remote_name {
            tracing::debug!(remote_name, "Looking for remote");
        }

        for remote in self.remotes()? {
            if let Some(remote_name) = gerrit_remote_name {
                if remote_name != remote {
                    tracing::debug!(remote, "Skipping remote");
                    continue;
                }
            }

            let url = self.remote_url(&remote)?;

            tried.push(url.clone());

            match GerritGitRemote::from_remote(&remote, &url) {
                Ok(gerrit) => {
                    return Ok(gerrit);
                }
                Err(error) => {
                    tracing::debug!(remote, url, %error, "Failed to parse remote URL");
                }
            }
        }

        Err(miette!("Failed to parse Gerrit configuration from Git remotes. Tried to parse these remotes:\n{}", format_bulleted_list(tried)))
    }

    pub fn cherry_pick(&self, commitish: &str) -> miette::Result<()> {
        self.command()
            .args(["cherry-pick", "--ff", commitish])
            .status_checked()
            .map(|_| ())
            .into_diagnostic()?;
        Ok(())
    }

    pub fn fetch(&self, remote: &str) -> miette::Result<()> {
        self.command()
            .args(["fetch", remote])
            .status_checked()
            .map(|_| ())
            .into_diagnostic()
    }

    pub fn checkout(&self, commitish: &str) -> miette::Result<()> {
        self.command()
            .args(["checkout", commitish])
            .status_checked()
            .map(|_| ())
            .into_diagnostic()
    }

    pub fn checkout_quiet(&self, commitish: &str) -> miette::Result<()> {
        self.command()
            .args(["checkout", commitish])
            .output_checked_utf8()
            .map(|_| ())
            .into_diagnostic()
    }

    pub fn detach_head(&self) -> miette::Result<()> {
        self.command()
            .args(["checkout", "--detach"])
            .output_checked_utf8()
            .into_diagnostic()?;
        Ok(())
    }

    /// Get the `HEAD` commit hash.
    pub fn get_head(&self) -> miette::Result<CommitHash> {
        self.rev_parse("HEAD")
    }

    /// Get the `.git` directory path.
    pub fn get_git_dir(&self) -> miette::Result<Utf8PathBuf> {
        self.command()
            .args(["rev-parse", "--git-dir"])
            .output_checked_utf8()
            .into_diagnostic()
            .map(|output| Utf8PathBuf::from(output.stdout.trim()))
    }

    pub fn rev_parse(&self, commitish: &str) -> miette::Result<CommitHash> {
        Ok(CommitHash::new(
            self.command()
                .args(["rev-parse", commitish])
                .output_checked_utf8()
                .into_diagnostic()?
                .stdout
                .trim()
                .to_owned(),
        ))
    }
}

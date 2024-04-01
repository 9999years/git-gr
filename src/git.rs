use std::io::stdout;
use std::process::Command;

use command_error::CommandExt;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;

use crate::format_bulleted_list;
use crate::gerrit::Gerrit;

/// `git` CLI wrapper.
#[derive(Debug)]
pub struct Git {}

impl Git {
    pub fn new() -> Self {
        Self {}
    }

    /// Get a `git` command.
    pub fn command(&self) -> Command {
        Command::new("git")
    }

    /// Push to a `refs/for/{branch}` ref.
    pub fn gerrit_push(&self, remote: &str, branch: &str) -> miette::Result<()> {
        self.command()
            .args([remote, &format!("refs/for/{branch}")])
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

    /// Get the (fetch) URL for the given remote.
    pub fn remote_url(&self, remote: &str) -> miette::Result<String> {
        Ok(self
            .command()
            .args(["remote", "get-url", &remote])
            .output_checked_utf8()
            .into_diagnostic()
            .wrap_err("Failed to get Git remote URL")?
            .stdout
            .trim()
            .to_owned())
    }

    pub fn default_branch(&self, remote: &str) -> miette::Result<String> {
        let full_branch = self
            .command()
            .args([
                "symbolic-ref",
                "--short",
                &format!("refs/remotes/{remote}/HEAD"),
            ])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        full_branch
            .strip_prefix(remote)
            .and_then(|branch| branch.strip_prefix('/'))
            .ok_or_else(|| {
                miette!("Failed to parse branch; expected \"{remote}/BRANCH\", got {full_branch:?}")
            })
            .map(|branch| branch.to_owned())
    }

    pub fn gerrit(&self, gerrit_remote_name: Option<&str>) -> miette::Result<Gerrit> {
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

            if !remote.contains("gerrit") && !url.contains("gerrit") {
                // Shrugs!
                tracing::debug!(remote, url, "Skipping remote");
                continue;
            }

            tried.push(url.clone());

            match Gerrit::parse_from_remote_url(&url) {
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
}

use std::ops::Deref;
use std::ops::DerefMut;
use std::process::Command;
use std::sync::OnceLock;

use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use regex::Regex;
use serde::de::DeserializeOwned;
use utf8_command::Utf8Output;

use crate::change_number::ChangeNumber;
use crate::current_patch_set::CurrentPatchSet;
use crate::gerrit_query::GerritQuery;
use crate::git::Git;
use crate::query_result::ChangeCurrentPatchSet;
use crate::query_result::QueryResult;

/// Gerrit SSH client wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gerrit {
    username: String,
    host: String,
    port: u16,
    project: String,
}

impl Gerrit {
    /// Parse a Gerrit configuration from a Git remote URL.
    pub fn parse_from_remote_url(url: &str) -> miette::Result<Self> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let captures = RE
            .get_or_init(|| {
                // ssh://USER@HOST:PORT/PROJECT
                Regex::new(
                    r"(?x)
                    ^
                    ssh://
                    (?P<user>[[:word:]]+)
                    @
                    (?P<host>[[:word:]][[:word:].]*)
                    :
                    (?P<port>[0-9]+)
                    /
                    (?P<project>[[:word:].]+)
                    $",
                )
                .expect("Regex parses")
            })
            .captures(url);
        match captures {
            Some(captures) => {
                let port = &captures["port"];
                let port = port.parse().into_diagnostic().wrap_err_with(|| {
                    format!("Failed to parse port `{port}` from Git remote: {url}")
                })?;

                Ok(Self {
                    username: captures["user"].to_owned(),
                    host: captures["host"].to_owned(),
                    port,
                    project: captures["project"].to_owned(),
                })
            }
            None => Err(miette!("Could not parse Git remote as Gerrit URL: {url}")),
        }
    }

    fn git(&self) -> Git {
        Git {}
    }

    /// The `ssh` destination to connect to.
    pub fn connect_to(&self) -> String {
        format!("ssh://{}@{}:{}", self.username, self.host, self.port)
    }

    pub fn remote(&self) -> String {
        // TODO: Get remote name.
        format!("{}/{}", self.connect_to(), self.project)
    }

    /// A `gerrit` command to run on the remote.
    pub fn command(&self, args: impl IntoIterator<Item = impl AsRef<str>>) -> Command {
        let mut cmd = Command::new("ssh");
        cmd.args([&self.connect_to(), "gerrit"]);
        cmd.args(
            args.into_iter()
                .map(|arg| shell_words::quote(arg.as_ref()).into_owned()),
        );
        cmd
    }

    pub fn query<T: DeserializeOwned>(&self, query: GerritQuery) -> miette::Result<QueryResult<T>> {
        self.command(query.into_args())
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    match QueryResult::from_stdout(&context.output().stdout) {
                        Ok(value) => Ok(value),
                        Err(error) => Err(context.error_msg(error)),
                    }
                } else {
                    Err(context.error())
                }
            })
            .into_diagnostic()
    }

    fn current_patch_set(&self, change: ChangeNumber) -> miette::Result<CurrentPatchSet> {
        let mut result = self.query::<ChangeCurrentPatchSet>(
            GerritQuery::new(change.to_string()).current_patch_set(),
        )?;
        Ok(result
            .changes
            .pop()
            .ok_or_else(|| miette!("Didn't find change {change}"))?
            .current_patch_set)
    }

    fn cl_ref(&self, change: ChangeNumber) -> miette::Result<String> {
        Ok(self.current_patch_set(change)?.ref_name)
    }

    /// Checkout a CL.
    ///
    /// TODO: Should maybe switch to a branch first?
    pub fn checkout_cl(&self, change: ChangeNumber) -> miette::Result<()> {
        let git = self.git();
        git.command()
            .args(["fetch", &self.remote(), &self.cl_ref(change)?])
            .status_checked()
            .into_diagnostic()?;
        // Seriously, `git fetch` doesn't write the fetched ref anywhere but `FETCH_HEAD`?
        git.command()
            .args(["checkout", "FETCH_HEAD"])
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }
}

/// A [`Gerrit`] client tied to a specific Git remote.
#[derive(Debug, Clone)]
pub struct GerritGitRemote {
    pub remote: String,
    inner: Gerrit,
}

impl GerritGitRemote {
    pub fn from_remote(remote: &str, url: &str) -> miette::Result<Self> {
        Gerrit::parse_from_remote_url(url).map(|inner| Self {
            remote: remote.to_owned(),
            inner,
        })
    }
}

impl Deref for GerritGitRemote {
    type Target = Gerrit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for GerritGitRemote {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_gerrit_parse_remote_url() {
        assert_eq!(
            Gerrit::parse_from_remote_url("ssh://rbt@ooga.booga.systems:2022/ouppy").unwrap(),
            Gerrit {
                username: "rbt".to_owned(),
                host: "ooga.booga.systems".to_owned(),
                port: 2022,
                project: "ouppy".to_owned()
            }
        );
    }
}

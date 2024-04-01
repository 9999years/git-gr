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
use crate::gerrit_query::GerritQuery;
use crate::git::Git;

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

    pub fn query<T: DeserializeOwned>(&self, query: GerritQuery) -> miette::Result<T> {
        self.command(query.into_args())
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if context.status().success() {
                    match serde_json::from_str(&context.output().stdout) {
                        Ok(value) => Ok(value),
                        Err(error) => Err(context.error_msg(error)),
                    }
                } else {
                    Err(context.error())
                }
            })
            .into_diagnostic()
    }

    fn cl_ref(&self, id: ChangeNumber) -> String {
        let patch_number = todo!();
        id.git_ref(patch_number)
    }

    /// Checkout a CL.
    pub fn checkout_cl(&self, id: ChangeNumber) -> miette::Result<()> {
        // git fetch ssh://rbt@gerrit.lix.systems:2022/lix refs/changes/85/685/5 && git checkout FETCH_HEAD
        let git = self.git();
        git.command()
            .args(["fetch", &self.remote(), &self.cl_ref(id)])
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

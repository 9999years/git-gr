use std::fmt::Display;
use std::ops::Deref;
use std::sync::OnceLock;

use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use regex::Regex;

use crate::gerrit_host::GerritHost;

/// A [`GerritHost`] with a project name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GerritProject {
    host: GerritHost,
    pub project: String,
}

impl GerritProject {
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
                    host: GerritHost {
                        username: captures["user"].to_owned(),
                        host: captures["host"].to_owned(),
                        port,
                    },
                    project: captures["project"].to_owned(),
                })
            }
            None => Err(miette!("Could not parse Git remote as Gerrit URL: {url}")),
        }
    }

    pub fn remote_url(&self) -> String {
        format!("{}/{}", self.connect_to(), self.project)
    }
}

impl Display for GerritProject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.remote_url())
    }
}

impl Deref for GerritProject {
    type Target = GerritHost;

    fn deref(&self) -> &Self::Target {
        &self.host
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_gerrit_parse_remote_url() {
        assert_eq!(
            GerritProject::parse_from_remote_url("ssh://rbt@ooga.booga.systems:2022/ouppy")
                .unwrap(),
            GerritProject {
                host: GerritHost {
                    username: "rbt".to_owned(),
                    host: "ooga.booga.systems".to_owned(),
                    port: 2022,
                },
                project: "ouppy".to_owned(),
            }
        );
    }
}

use std::fmt::Debug;
use std::ops::Deref;
use std::ops::DerefMut;
use std::process::Command;
use std::sync::OnceLock;

use calm_io::stdoutln;
use comfy_table::Attribute;
use comfy_table::Cell;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use regex::Regex;
use reqwest::Method;
use secrecy::ExposeSecret;
use secrecy::SecretString;
use serde::de::DeserializeOwned;
use tracing::instrument;
use utf8_command::Utf8Output;

use crate::change::Change;
use crate::change::TimestampFormat;
use crate::change_number::ChangeNumber;
use crate::dependency_graph::DependencyGraph;
use crate::format_bulleted_list;
use crate::git::Git;
use crate::query::Query;
use crate::query::QueryOptions;
use crate::query_result::ChangeCurrentPatchSet;
use crate::query_result::ChangeDependencies;
use crate::query_result::ChangeSubmitRecords;
use crate::query_result::QueryResult;
use crate::restack::restack;
use crate::restack::restack_abort;
use crate::restack_push::restack_push;
use crate::tmpdir::ssh_control_path;

/// Gerrit SSH client wrapper.
#[derive(Clone)]
pub struct Gerrit {
    username: String,
    host: String,
    port: u16,
    project: String,

    /// Password for the REST API.
    ///
    /// Generated with `gerrit set-account --generate-http-password`.
    http_password: Option<SecretString>,
    http_client: Option<reqwest::blocking::Client>,
}

impl Debug for Gerrit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Gerrit").field(&self.remote()).finish()
    }
}

impl PartialEq for Gerrit {
    fn eq(&self, other: &Self) -> bool {
        self.username == other.username
            && self.host == other.host
            && self.port == other.port
            && self.project == other.project
    }
}

impl Eq for Gerrit {}

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
                    http_password: None,
                    http_client: None,
                })
            }
            None => Err(miette!("Could not parse Git remote as Gerrit URL: {url}")),
        }
    }

    pub fn git(&self) -> Git {
        Git {}
    }

    /// The `ssh` destination to connect to.
    pub fn connect_to(&self) -> String {
        format!("ssh://{}@{}:{}", self.username, self.host, self.port)
    }

    /// Given an endpoint path, format an HTTP request URL.
    fn endpoint(&self, endpoint: &str) -> String {
        let endpoint = endpoint.trim_start_matches('/');
        format!("https://{}/a/{endpoint}", self.host)
    }

    pub fn remote(&self) -> String {
        // TODO: Get remote name.
        format!("{}/{}", self.connect_to(), self.project)
    }

    /// A `gerrit` command to run on the remote.
    pub fn command(&self, args: impl IntoIterator<Item = impl AsRef<str>>) -> Command {
        let mut cmd = Command::new("ssh");
        cmd.args([
            // Persist sessions in the background to speed up subsequent `ssh` calls.
            "-o",
            "ControlMaster=auto",
            "-o",
            &format!(
                "ControlPath={}",
                ssh_control_path(&format!(
                    "git-gr-ssh-{}-{}-{}",
                    self.username, self.host, self.port
                ))
            ),
            "-o",
            "ControlPersist=120",
            &self.connect_to(),
            "gerrit",
        ]);
        cmd.args(
            args.into_iter()
                .map(|arg| shell_words::quote(arg.as_ref()).into_owned()),
        );
        cmd
    }

    pub fn query<T: DeserializeOwned>(
        &self,
        query: QueryOptions,
    ) -> miette::Result<QueryResult<T>> {
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

    pub fn get_change<'a>(&self, change: impl Into<Query<'a>>) -> miette::Result<Change> {
        let change = change.into();
        let mut result = self.query::<Change>(QueryOptions::new(&change))?;
        result
            .changes
            .pop()
            .ok_or_else(|| miette!("Didn't find change {change}"))
    }

    pub fn get_current_patch_set<'a>(
        &self,
        change: impl Into<Query<'a>>,
    ) -> miette::Result<ChangeCurrentPatchSet> {
        let change = change.into();
        let mut result =
            self.query::<ChangeCurrentPatchSet>(QueryOptions::new(&change).current_patch_set())?;
        result
            .changes
            .pop()
            .ok_or_else(|| miette!("Didn't find change {change}"))
    }

    pub fn dependencies<'a>(
        &self,
        change: impl Into<Query<'a>>,
    ) -> miette::Result<ChangeDependencies> {
        let change = change.into();
        let mut result =
            self.query::<ChangeDependencies>(QueryOptions::new(&change).dependencies())?;
        result
            .changes
            .pop()
            .ok_or_else(|| miette!("Didn't find change {change}"))
    }

    pub fn dependency_graph<'a>(
        &self,
        change: impl Into<Query<'a>>,
    ) -> miette::Result<DependencyGraph> {
        let change = change.into();
        let change = self.get_change(change)?;
        DependencyGraph::traverse(self, change.number)
    }

    fn cl_ref<'a>(&self, change: impl Into<Query<'a>>) -> miette::Result<String> {
        let change = change.into();
        Ok(self
            .get_current_patch_set(change)?
            .current_patch_set
            .ref_name)
    }

    /// Fetch a CL.
    ///
    /// Returns the Git ref of the fetched patchset.
    pub fn fetch_cl<'a>(&self, change: impl Into<Query<'a>>) -> miette::Result<String> {
        let change = change.into();
        let git = self.git();
        git.command()
            .args(["fetch", &self.remote(), &self.cl_ref(change)?])
            .status_checked()
            .into_diagnostic()?;
        // Seriously, `git fetch` doesn't write the fetched ref anywhere but `FETCH_HEAD`?
        git.rev_parse("FETCH_HEAD")
    }

    /// Fetch a CL without forwarding output to the user's terminal.
    ///
    /// Returns the Git ref of the fetched patchset.
    pub fn fetch_cl_quiet<'a>(&self, change: impl Into<Query<'a>>) -> miette::Result<String> {
        let change = change.into();
        let git = self.git();
        git.command()
            .args(["fetch", &self.remote(), &self.cl_ref(change)?])
            .output_checked_utf8()
            .into_diagnostic()?;
        // Seriously, `git fetch` doesn't write the fetched ref anywhere but `FETCH_HEAD`?
        git.rev_parse("FETCH_HEAD")
    }

    /// Checkout a CL.
    pub fn checkout_cl<'a>(&self, change: impl Into<Query<'a>>) -> miette::Result<()> {
        let change = change.into();
        let git_ref = self.fetch_cl(change)?;
        let git = self.git();
        git.command()
            .args(["checkout", &git_ref])
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }

    /// Checkout a CL at a specific patchset.
    pub fn checkout_cl_patchset(&self, change: ChangeNumber, patchset: u32) -> miette::Result<()> {
        let git = self.git();
        git.command()
            .args(["fetch", &self.remote(), &change.git_ref(patchset)])
            .output_checked_utf8()
            .into_diagnostic()?;
        git.checkout("FETCH_HEAD")?;
        Ok(())
    }

    pub fn restack_abort(&self) -> miette::Result<()> {
        restack_abort(&self.git())
    }

    pub fn up(&self) -> miette::Result<()> {
        let git = self.git();
        let change_id = git
            .change_id("HEAD")
            .wrap_err("Failed to get Change-Id for HEAD")?;
        let dependencies = self
            .dependencies(&change_id)
            .wrap_err("Failed to get change dependencies")?
            .filter_unmerged(self)?;
        let mut needed_by = dependencies.needed_by_numbers();
        let needed_by = match needed_by.len() {
            0 => {
                return Err(miette!(
                    "Change {} isn't needed by any changes",
                    dependencies.change.number
                ));
            }
            1 => needed_by.pop_first().expect("Length was checked"),
            _ => {
                return Err(miette!(
                        "Change {} is needed by multiple changes; use `git-gr checkout {}` to pick one:\n{}",
                        dependencies.change.number,
                        dependencies.change.number,
                        format_bulleted_list(needed_by)
                    ));
            }
        };
        self.checkout_cl(needed_by)?;
        Ok(())
    }

    pub fn top(&self) -> miette::Result<()> {
        let git = self.git();
        let change_id = git
            .change_id("HEAD")
            .wrap_err("Failed to get Change-Id for HEAD")?;
        let change = self.get_change(&change_id)?.number;
        let mut next = change;

        loop {
            let mut needed_by = self
                .dependencies(next)
                .wrap_err("Failed to get change dependencies")?
                .filter_unmerged(self)?
                .needed_by_numbers();

            next = match needed_by.len() {
                0 => {
                    break;
                }
                1 => needed_by.pop_first().expect("Length was checked"),
                _ => {
                    return Err(miette!(
                        "Change {} is needed by multiple changes; use `git-gr checkout {}` to pick one:\n{}",
                        next,
                        next,
                        format_bulleted_list(needed_by)
                    ));
                }
            };
        }
        self.checkout_cl(next)?;
        Ok(())
    }

    pub fn down(&self) -> miette::Result<()> {
        let git = self.git();
        let change_id = git
            .change_id("HEAD")
            .wrap_err("Failed to get Change-Id for HEAD")?;
        let dependencies = self
            .dependencies(&change_id)
            .wrap_err("Failed to get change dependencies")?
            .filter_unmerged(self)?;
        let mut depends_on = dependencies.depends_on_numbers();
        let depends_on = match depends_on.len() {
            0 => {
                return Err(miette!(
                    "Change {} doesn't depend on any changes",
                    dependencies.change.number
                ));
            }
            1 => depends_on.pop_first().expect("Length was checked"),
            _ => {
                return Err(miette!(
                        "Change {} depends on multiple changes, use `git-gr checkout {}` to pick one:\n{}",
                        dependencies.change.number,
                        dependencies.change.number,
                        format_bulleted_list(&depends_on)
                    ));
            }
        };
        self.checkout_cl(depends_on)?;
        Ok(())
    }

    pub fn print_query(&self, query: String) -> miette::Result<()> {
        let results = self
            .query::<ChangeSubmitRecords>(QueryOptions::new(query).no_limit().submit_records())?;

        let timestamp_format = if std::env::var("GIT_GR_24_HOUR_TIME")
            .map(|value| !value.is_empty())
            .unwrap_or(false)
        {
            TimestampFormat::TwentyFourHour
        } else {
            TimestampFormat::TwelveHour
        };

        let mut table = comfy_table::Table::new();
        table
            .load_preset(comfy_table::presets::NOTHING)
            .set_content_arrangement(comfy_table::ContentArrangement::DynamicFullWidth)
            .set_header(
                [
                    "#", "Subject",
                    // 5-letter abbreviation doesn't make the column too wide for short
                    // timestamps like `21:30` or `04-30`.
                    "Updat", "Owner", "Status", "",
                ]
                .map(|cell| {
                    Cell::new(cell)
                        .add_attribute(Attribute::Bold)
                        .add_attribute(Attribute::Underlined)
                }),
            );

        for change in &results.changes {
            table.add_row([
                Cell::new(change.change.number).add_attribute(Attribute::Bold),
                Cell::new(change.change.subject.clone().unwrap_or_default()),
                change.change.last_updated_cell(timestamp_format)?,
                Cell::new(change.change.owner.username.clone()),
                change.change.status_cell(),
                change.ready_cell(),
            ]);
        }

        // Change numbers.
        table
            .column_mut(0)
            .expect("First column exists")
            .set_cell_alignment(comfy_table::CellAlignment::Right);

        // Updated times.
        table
            .column_mut(2)
            .expect("Third column exists")
            .set_cell_alignment(comfy_table::CellAlignment::Right);

        let _ = stdoutln!("{table}");

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

    pub fn restack_this(&self) -> miette::Result<()> {
        let change_id = self
            .git()
            .change_id("HEAD")
            .wrap_err("Failed to get Change-Id for HEAD")?;
        let change = self.get_change(&change_id)?;
        let dependencies = self
            .dependencies(&change_id)
            .wrap_err("Failed to get change dependencies")?
            .filter_unmerged(self)?;
        let mut depends_on = dependencies.depends_on_numbers();
        let depends_on = match depends_on.len() {
            0 => {
                return Err(miette!(
                    "Change {} doesn't depend on any changes",
                    dependencies.change.number
                ));
            }
            1 => depends_on.pop_first().expect("Length was checked"),
            _ => {
                return Err(miette!(
                        "Change {} depends on multiple changes, use `git-gr checkout {}` to pick one:\n{}",
                        dependencies.change.number,
                        dependencies.change.number,
                        format_bulleted_list(&depends_on)
                    ));
            }
        };
        let depends_on = self.get_current_patch_set(depends_on)?;
        tracing::info!(
            "Rebasing {} on {}: {}",
            change.number,
            depends_on.change.number,
            depends_on.current_patch_set.revision
        );
        self.git()
            .command()
            .args(["rebase", &depends_on.current_patch_set.revision])
            .status_checked()
            .into_diagnostic()?;
        Ok(())
    }

    pub fn push(&self, branch: Option<String>, target: Option<String>) -> miette::Result<()> {
        let git = self.git();
        let target = match target {
            Some(target) => target,
            None => git.default_branch(&self.remote)?,
        };
        let branch = match branch {
            Some(branch) => branch,
            None => "HEAD".to_owned(),
        };
        git.gerrit_push(&self.remote, &branch, &target)?;
        Ok(())
    }

    pub fn restack(&self, branch: &str) -> miette::Result<()> {
        restack(self, branch)
    }

    pub fn restack_continue(&self) -> miette::Result<()> {
        self.restack("HEAD")
    }

    pub fn restack_push(&self) -> miette::Result<()> {
        restack_push(self)
    }

    /// Ensure that this object has an HTTP password set.
    pub fn generate_http_password(&mut self) -> miette::Result<()> {
        if self.http_password.is_some() {
            return Ok(());
        }

        let output = self
            .command(["set-account", &self.username, "--generate-http-password"])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout;

        static RE: OnceLock<Regex> = OnceLock::new();
        let captures = RE
            .get_or_init(|| {
                Regex::new(
                    r"(?xm)
                    ^
                    New\ password:
                    \ (?P<password>[a-zA-Z0-9/+=]+)
                    $",
                )
                .expect("Regex parses")
            })
            .captures(&output);

        match captures {
            Some(captures) => {
                self.http_password = Some(SecretString::new(captures["password"].to_owned()));
                Ok(())
            }
            None => Err(miette!("Could not parse Gerrit HTTP password: {output:?}")),
        }
    }

    /// Ensure that `http_password` and `http_client` are populated.
    fn http_ensure(&mut self) -> miette::Result<()> {
        self.generate_http_password()?;

        if self.http_client.is_none() {
            self.http_client = Some(reqwest::blocking::Client::new());
        }

        Ok(())
    }

    #[instrument()]
    pub fn http_request(&mut self, method: Method, endpoint: &str) -> miette::Result<String> {
        self.http_ensure()?;

        let url = self.endpoint(endpoint);

        let response = self
            .http_client
            .as_ref()
            .expect("http_ensure should construct an HTTP client")
            .request(method.clone(), &url)
            .basic_auth(
                &self.username,
                self.http_password
                    .as_ref()
                    .map(|password| password.expose_secret()),
            )
            .send()
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to {method} {url}"))?;

        if response.status().is_success() {
            let body = response
                .text()
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to get response body for {url}"))?;

            Ok(body
                .strip_prefix(")]}'\n")
                .map(|body| body.to_owned())
                .unwrap_or(body))
        } else {
            Err(miette!(
                "{method} {url} failed with status {}:\n{}",
                response.status(),
                response
                    .text()
                    .unwrap_or_else(|error| { format!("Failed to get response body: {error}") })
            ))
        }
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
                project: "ouppy".to_owned(),
                http_password: None,
                http_client: None
            }
        );
    }
}

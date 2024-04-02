use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
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

use crate::chain::Chain;
use crate::change_number::ChangeNumber;
use crate::git::Git;
use crate::query::Query;
use crate::query::QueryOptions;
use crate::query_result::Change;
use crate::query_result::ChangeCurrentPatchSet;
use crate::query_result::ChangeDependencies;
use crate::query_result::QueryResult;
use crate::tmpdir::ssh_control_path;

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
        cmd.args([
            // Persist sessions in the background to speed up subsequent `ssh` calls.
            "-o",
            "ControlMaster=auto",
            "-o",
            &format!(
                "ControlPath={}",
                ssh_control_path(&format!(
                    "gayrat-ssh-{}-{}-{}",
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

    pub fn dependency_graph<'a>(&self, change: impl Into<Query<'a>>) -> miette::Result<Chain> {
        let change = change.into();
        let change = self.get_change(change)?;
        Chain::new(self, change.number)
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
        Ok(git
            .command()
            .args(["rev-parse", "FETCH_HEAD"])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout
            .trim()
            .to_owned())
    }

    /// Checkout a CL.
    ///
    /// TODO: Should maybe switch to a branch first?
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

    pub fn restack(&self) -> miette::Result<()> {
        let git = self.git();
        let change_id = git
            .change_id("HEAD")
            .wrap_err("Failed to get Change-Id for HEAD")?;
        let gerrit = git.gerrit(None)?;
        git.fetch(&gerrit.remote)?;
        let mut chain = gerrit.dependency_graph(&change_id)?;

        // CL to Git commit hash map representing updated refs after rebase.
        let mut updated_refs = BTreeMap::<ChangeNumber, String>::new();

        // TODO: Serialize graph to disk so we can continue when there's merge
        // conflicts.
        let roots = chain.depends_on_roots();
        for root in &roots {
            let mut seen = BTreeSet::new();
            let mut queue = VecDeque::new();
            queue.push_front(*root);

            while !queue.is_empty() {
                let change = queue.pop_back().expect("Length is checked");

                if roots.contains(&change) {
                    // Change is root, rebase on target branch.
                    let change = gerrit.get_current_patch_set(change)?;
                    tracing::info!(
                        "Restacking change {} ({}) on {}",
                        change.change.number,
                        change.change.subject.unwrap_or_default(),
                        change.change.branch
                    );
                    git.rebase(&format!("{}/{}", gerrit.remote, change.change.branch))?;
                    updated_refs.insert(change.change.number, git.get_head()?);
                } else {
                    // Change is not root, rebase on parent.
                    let parent = chain.dependencies.depends_on(change).ok_or_else(|| {
                        miette!("Change does not have parent to rebase onto: {change}")
                    })?;
                    let parent_ref = match updated_refs.get(&parent) {
                        Some(parent_ref) => parent_ref.to_owned(),
                        None => gerrit.fetch_cl(parent)?,
                    };
                    let parent = gerrit.get_change(parent)?;
                    gerrit.checkout_cl(change)?;
                    let change = gerrit.get_change(change)?;
                    tracing::info!(
                        "Restacking change {} ({}) on {} ({})",
                        change.number,
                        change.subject.unwrap_or_default(),
                        parent.number,
                        parent.subject.unwrap_or_default(),
                    );
                    git.rebase(&parent_ref)?;
                    updated_refs.insert(change.number, git.get_head()?);
                }

                let reverse_dependencies = chain.dependencies.needed_by(change);

                for needed_by in reverse_dependencies {
                    if !seen.contains(needed_by) {
                        seen.insert(*needed_by);
                        queue.push_front(*needed_by);
                    }
                }
            }
        }
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

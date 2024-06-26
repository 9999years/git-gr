use std::fmt::Debug;
use std::io::BufWriter;
use std::io::Write;
use std::ops::Deref;
use std::ops::DerefMut;
use std::process::Command;
use std::sync::OnceLock;

use cached::IOCached;
use camino::Utf8Path;
use comfy_table::Attribute;
use comfy_table::Cell;
use command_error::CommandExt;
use command_error::OutputContext;
use fs_err::File;
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

use crate::cache::CacheKey;
use crate::cache::CacheValue;
use crate::cache::GerritCache;
use crate::change::Change;
use crate::change::TimestampFormat;
use crate::change_key::ChangeKey;
use crate::change_number::ChangeNumber;
use crate::cli::RestackContinue;
use crate::commit_hash::CommitHash;
use crate::current_exe::current_exe;
use crate::dependency_graph::DependencyGraph;
use crate::endpoint::Endpoint;
use crate::format_bulleted_list;
use crate::gerrit_project::GerritProject;
use crate::git::Git;
use crate::patchset::ChangePatchset;
use crate::query::QueryOptions;
use crate::query_result::QueryResult;
use crate::related_changes_info::RelatedChangesInfo;
use crate::restack::format_git_rebase_todo;
use crate::restack::restack;
use crate::restack::restack_abort;
use crate::restack_push::restack_push;
use crate::tmpdir::ssh_control_path;

/// Gerrit SSH client wrapper.
pub struct Gerrit {
    host: GerritProject,

    /// Password for the REST API.
    ///
    /// Generated with `gerrit set-account --generate-http-password`.
    http_password: Option<SecretString>,
    http_client: Option<reqwest::blocking::Client>,

    cache: GerritCache,
}

impl Debug for Gerrit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Gerrit")
            .field(&self.host.to_string())
            .finish()
    }
}

impl Gerrit {
    pub fn new(host: GerritProject) -> miette::Result<Self> {
        let cache = GerritCache::new(&host)?;
        Ok(Self {
            host,
            http_password: None,
            http_client: None,
            cache,
        })
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear_cache();
    }

    pub fn deattach_cache(&mut self) {
        self.cache.deattach_cache();
    }

    pub fn attach_cache(&mut self) -> miette::Result<()> {
        self.cache.attach_cache(&self.host)?;
        Ok(())
    }

    pub fn git(&self) -> Git {
        Git {}
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
                    self.host.username, self.host.host, self.host.port
                ))
            ),
            "-o",
            "ControlPersist=120",
            &self.host.connect_to(),
            "gerrit",
        ]);
        cmd.args(
            args.into_iter()
                .map(|arg| shell_words::quote(arg.as_ref()).into_owned()),
        );
        cmd
    }

    pub fn query(&self, query: QueryOptions) -> miette::Result<QueryResult<Change>> {
        let key = CacheKey::Query(query.query_string().to_owned());
        if let Some(value) = self.cache.cache_get(&key).into_diagnostic()? {
            return match value {
                CacheValue::Query(result) => Ok(result),
                _ => Err(miette!("Cached value isn't a set of changes: {value:?}")),
            };
        }

        let result = self
            .command(query.into_args())
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
            .into_diagnostic()?;

        self.cache
            .cache_set(key, CacheValue::Query(result.clone()))
            .into_diagnostic()?;

        Ok(result)
    }

    fn cache_change(&self, change: Change) -> miette::Result<()> {
        let number = change.number;
        let id = change.id.clone();
        let value = CacheValue::Change(Box::new(change));
        self.cache
            .cache_set(CacheKey::Change(number), value.clone())
            .into_diagnostic()?;
        self.cache
            .cache_set(CacheKey::ChangeId(id), value)
            .into_diagnostic()?;

        Ok(())
    }

    pub fn get_change(&self, change: impl Into<ChangeKey>) -> miette::Result<Change> {
        let change: ChangeKey = change.into();
        if let Some(value) = self
            .cache
            .cache_get(&change.clone().into())
            .into_diagnostic()?
        {
            return match value {
                CacheValue::Change(change) => Ok(*change),
                _ => Err(miette!("Cached value isn't a change: {value:?}")),
            };
        }

        let query = change.to_string();
        let result = self
            .query(
                QueryOptions::new(query.clone())
                    .current_patch_set()
                    .dependencies()
                    .submit_records(),
            )?
            .changes
            .pop()
            .ok_or_else(|| miette!("Didn't find change {query}"))?;
        self.cache_change(result.clone())?;
        Ok(result)
    }

    pub fn dependency_graph(&mut self, root: ChangeNumber) -> miette::Result<DependencyGraph> {
        DependencyGraph::traverse(self, root)
    }

    pub fn git_sequence_editor(&self) -> miette::Result<String> {
        let exe = current_exe()?;
        let exe = shell_words::quote(exe.as_str());
        Ok(format!("{exe} restack write-todo"))
    }

    /// Fetch a CL.
    ///
    /// Returns the Git ref of the fetched patchset.
    pub fn fetch_cl(&self, change: ChangePatchset) -> miette::Result<CommitHash> {
        if let Some(value) = self
            .cache
            .cache_get(&CacheKey::Fetch(change))
            .into_diagnostic()?
        {
            return match value {
                CacheValue::Fetch(hash) => Ok(hash),
                _ => Err(miette!("Cached value isn't a change: {value:?}")),
            };
        }

        let git = self.git();
        git.command()
            .args(["fetch", &self.host.remote_url(), &change.git_ref()])
            .output_checked_utf8()
            .into_diagnostic()?;

        // Seriously, `git fetch` doesn't write the fetched ref anywhere but `FETCH_HEAD`?
        git.rev_parse("FETCH_HEAD")
    }

    /// Checkout a CL.
    pub fn checkout_cl(&self, change: ChangePatchset) -> miette::Result<()> {
        let git = self.git();
        git.command()
            .args(["checkout", &self.fetch_cl(change)?])
            .status_checked()
            .into_diagnostic()?;
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
        let change = self
            .get_change(change_id)
            .wrap_err("Failed to get change dependencies")?
            .filter_unmerged(self)?;
        let mut needed_by = change.needed_by_numbers();
        let needed_by = match needed_by.len() {
            0 => {
                return Err(miette!(
                    "Change {} isn't needed by any changes",
                    change.number
                ));
            }
            1 => needed_by.pop_first().expect("Length was checked"),
            _ => {
                return Err(miette!(
                        "Change {} is needed by multiple changes; use `git-gr checkout {}` to pick one:\n{}",
                        change.number,
                        change.number,
                        format_bulleted_list(needed_by)
                    ));
            }
        };
        self.checkout_cl(self.get_change(needed_by)?.patchset())?;
        Ok(())
    }

    pub fn top(&self) -> miette::Result<()> {
        let git = self.git();
        let change_id = git
            .change_id("HEAD")
            .wrap_err("Failed to get Change-Id for HEAD")?;
        let mut next = self.get_change(change_id)?.filter_unmerged(self)?;

        loop {
            let mut needed_by = next.needed_by_numbers();

            next = match needed_by.len() {
                0 => {
                    break;
                }
                1 => self
                    .get_change(needed_by.pop_first().expect("Length was checked"))?
                    .filter_unmerged(self)?,
                _ => {
                    return Err(miette!(
                        "Change {} is needed by multiple changes; use `git-gr checkout {}` to pick one:\n{}",
                        next.number,
                        next.number,
                        format_bulleted_list(needed_by)
                    ));
                }
            };
        }
        self.checkout_cl(next.patchset())?;
        Ok(())
    }

    pub fn down(&self) -> miette::Result<()> {
        let git = self.git();
        let change_id = git
            .change_id("HEAD")
            .wrap_err("Failed to get Change-Id for HEAD")?;
        let change = self
            .get_change(change_id)
            .wrap_err("Failed to get change dependencies")?
            .filter_unmerged(self)?;
        let mut depends_on = change.depends_on_numbers();
        let depends_on = match depends_on.len() {
            0 => {
                return Err(miette!(
                    "Change {} doesn't depend on any changes",
                    change.number
                ));
            }
            1 => depends_on.pop_first().expect("Length was checked"),
            _ => {
                return Err(miette!(
                        "Change {} depends on multiple changes, use `git-gr checkout {}` to pick one:\n{}",
                        change.number,
                        change.number,
                        format_bulleted_list(&depends_on)
                    ));
            }
        };
        self.checkout_cl(self.get_change(depends_on)?.patchset())?;
        Ok(())
    }

    pub fn format_query_results(&self, query: String) -> miette::Result<comfy_table::Table> {
        let results = self.query(
            QueryOptions::new(query)
                .current_patch_set()
                .dependencies()
                .submit_records()
                .no_limit(),
        )?;

        // TODO: Make this configurable.
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
            .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
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
                Cell::new(change.number).add_attribute(Attribute::Bold),
                Cell::new(change.subject.clone().unwrap_or_default()),
                change.last_updated_cell(timestamp_format)?,
                Cell::new(change.owner.username.clone()),
                change.status_cell(),
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

        Ok(table)
    }

    pub fn rebase_interactive(&mut self, onto: &str) -> miette::Result<()> {
        self.deattach_cache();
        self.git()
            .rebase_interactive(&self.git_sequence_editor()?, onto)?;
        self.attach_cache()?;
        Ok(())
    }

    /// Ensure that this object has an HTTP password set.
    pub fn generate_http_password(&mut self) -> miette::Result<()> {
        if self.http_password.is_some() {
            return Ok(());
        }

        let output = self
            .command([
                "set-account",
                &self.host.username,
                "--generate-http-password",
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
    pub fn http_request(&mut self, method: Method, endpoint: &Endpoint) -> miette::Result<String> {
        let key = CacheKey::Api(endpoint.to_owned());
        if let Some(value) = self.cache.cache_get(&key).into_diagnostic()? {
            return match value {
                CacheValue::Api(response) => Ok(response),
                _ => Err(miette!("Cached value isn't an API response: {value:?}")),
            };
        }

        self.http_ensure()?;

        let url = self.host.endpoint(endpoint);

        let response = self
            .http_client
            .as_ref()
            .expect("http_ensure should construct an HTTP client")
            .request(method.clone(), &url)
            .basic_auth(
                &self.host.username,
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

            let body = body
                .strip_prefix(")]}'\n")
                .map(|body| body.to_owned())
                .unwrap_or(body);

            self.cache
                .cache_set(key, CacheValue::Api(body.clone()))
                .into_diagnostic()?;

            Ok(body)
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

    pub fn http_json<T: DeserializeOwned>(
        &mut self,
        method: Method,
        endpoint: &Endpoint,
    ) -> miette::Result<T> {
        let response = self.http_request(method, endpoint)?;
        serde_json::from_str(&response)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to deserialize JSON from HTTP request to {endpoint}"))
    }

    pub fn related_changes(
        &mut self,
        change_number: ChangeNumber,
        revision_number: Option<u32>,
    ) -> miette::Result<RelatedChangesInfo> {
        let revision = revision_number
            .map(|revision| revision.to_string())
            .unwrap_or_else(|| "current".to_owned());
        self.http_json::<RelatedChangesInfo>(
            Method::GET,
            &Endpoint::new(&format!(
                "changes/{}~{change_number}/revisions/{revision}/related?o=SUBMITTABLE",
                self.host.project
            )),
        )
    }
}

/// A [`Gerrit`] client tied to a specific Git remote.
#[derive(Debug)]
pub struct GerritGitRemote {
    pub remote: String,
    inner: Gerrit,
}

impl GerritGitRemote {
    pub fn from_remote(remote: &str, url: &str) -> miette::Result<Self> {
        Ok(Self {
            remote: remote.to_owned(),
            inner: GerritProject::parse_from_remote_url(url).and_then(Gerrit::new)?,
        })
    }

    pub fn restack_this(&mut self) -> miette::Result<()> {
        let change_id = self
            .git()
            .change_id("HEAD")
            .wrap_err("Failed to get Change-Id for HEAD")?;
        let change = self.get_change(change_id)?.filter_unmerged(self)?;
        let mut depends_on = change.depends_on_numbers();
        let depends_on = match depends_on.len() {
            0 => {
                return Err(miette!(
                    "Change {} doesn't depend on any changes",
                    change.number
                ));
            }
            1 => depends_on.pop_first().expect("Length was checked"),
            _ => {
                return Err(miette!(
                        "Change {} depends on multiple changes, use `git-gr checkout {}` to pick one:\n{}",
                        change.number,
                        change.number,
                        format_bulleted_list(&depends_on)
                    ));
            }
        };
        let depends_on = self.get_change(depends_on)?;
        tracing::info!(
            "Rebasing {} on {}: {}",
            change.number,
            depends_on.number,
            depends_on.current_patch_set.revision
        );
        let git = self.git();
        git.detach_head()?;
        self.rebase_interactive(&depends_on.current_patch_set.revision)?;
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
        let change_id = git.change_id(&branch)?;
        match self.get_change(change_id) {
            Ok(change) => {
                self.cache
                    .cache_remove(&CacheKey::Change(change.number))
                    .into_diagnostic()?;
                self.cache
                    .cache_remove(&CacheKey::ChangeId(change.id))
                    .into_diagnostic()?;
            }
            Err(error) => {
                tracing::debug!("Ignoring error from fetching change before pushing: {error}");
            }
        }
        Ok(())
    }

    pub fn restack(
        &mut self,
        branch: &str,
        options: Option<RestackContinue>,
    ) -> miette::Result<()> {
        restack(self, branch, options)
    }

    pub fn restack_continue(&mut self, options: RestackContinue) -> miette::Result<()> {
        self.restack("HEAD", Some(options))
    }

    pub fn restack_push(&self) -> miette::Result<()> {
        restack_push(self)
    }

    pub fn restack_write_git_rebase_todo(&mut self, path: &Utf8Path) -> miette::Result<()> {
        let mut file = BufWriter::new(File::create(path).into_diagnostic()?);

        let todo = format_git_rebase_todo(self)?;

        write!(file, "{todo}").into_diagnostic()?;

        Ok(())
    }

    pub fn format_chain(&mut self, query: Option<String>) -> miette::Result<String> {
        let git = self.git();
        let change_number = match query {
            Some(query) => self.get_change(query)?.number,
            None => {
                let change_id = git
                    .change_id("HEAD")
                    .wrap_err("Failed to get Change-Id for HEAD")?;
                self.get_change(change_id)?.number
            }
        };
        let mut graph = DependencyGraph::traverse(self, change_number)?;

        if let Some(todo) = crate::restack::get_todo(self)? {
            graph.format_tree(self, |change| {
                Ok(todo
                    .refs
                    .get(&change)
                    .into_iter()
                    .map(|update| update.to_string())
                    .collect())
            })
        } else if let Ok(todo) = crate::restack_push::maybe_get_todo(self)? {
            graph.format_tree(self, |change| {
                Ok(todo
                    .refs
                    .get(&change)
                    .into_iter()
                    .map(|update| update.to_string())
                    .collect())
            })
        } else {
            graph.format_tree(self, |_change| Ok(vec![]))
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

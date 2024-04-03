use std::borrow::Borrow;
use std::borrow::Cow;
use std::fmt::Display;

use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;

/// A `gerrit query`.
///
/// Efficiently models change number queries and string queries.
#[derive(Debug, Clone)]
pub enum Query<'a> {
    Change(ChangeNumber),
    String(Cow<'a, str>),
}

impl<'a> Default for Query<'a> {
    fn default() -> Self {
        Self::String(Default::default())
    }
}

impl<'a> Display for Query<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Query::Change(change) => change.fmt(f),
            Query::String(string) => string.fmt(f),
        }
    }
}

impl<'a> From<ChangeNumber> for Query<'a> {
    fn from(change: ChangeNumber) -> Self {
        Self::Change(change)
    }
}

impl<'a> From<String> for Query<'a> {
    fn from(query: String) -> Self {
        Self::String(Cow::Owned(query))
    }
}

impl<'a> From<&'a ChangeId> for Query<'a> {
    fn from(value: &'a ChangeId) -> Self {
        Self::String(Cow::Borrowed(value))
    }
}

impl<'a> From<&'a str> for Query<'a> {
    fn from(query: &'a str) -> Self {
        Self::String(Cow::Borrowed(query))
    }
}

impl<'a> From<&'a Query<'a>> for Query<'a> {
    fn from(query: &'a Query) -> Self {
        match query {
            Query::Change(change) => Query::Change(*change),
            Query::String(string) => Query::String(Cow::Borrowed(string.borrow())),
        }
    }
}

/// Options for performing a `gerrit query`.
///
/// Not modeled: `--deadline`.
#[derive(Default, Debug, Clone)]
pub struct QueryOptions<'a> {
    /// The query to execute.
    query: Query<'a>,
    /// Include information about all patch sets and approvals
    all_approvals: bool,
    /// Include all reviewers
    all_reviewers: bool,
    /// Include patch set and inline comments
    comments: bool,
    /// Include the full commit message for a change
    commit_message: bool,
    /// Include information about current patch set
    current_patch_set: bool,

    /// Include depends-on and needed-by information
    dependencies: bool,
    /// Include file list on patch sets
    files: bool,
    /// Return all results, overriding the default limit
    no_limit: bool,
    /// Include information about all patch sets
    patch_sets: bool,
    /// Number of changes to skip
    start: usize,
    /// Include submit and label status
    submit_records: bool,
}

impl<'a> QueryOptions<'a> {
    /// Construct query options wrapping the given string.
    pub fn new(query: impl Into<Query<'a>>) -> Self {
        Self {
            query: query.into(),
            all_approvals: false,
            all_reviewers: false,
            comments: false,
            commit_message: false,
            current_patch_set: false,
            dependencies: false,
            files: false,
            no_limit: false,
            patch_sets: false,
            start: 0,
            submit_records: false,
        }
    }

    /// Convert this query into CLI options, to be appended to `gerrit`.
    pub fn into_args(self) -> Vec<String> {
        let mut args = vec!["query".to_owned(), "--format".to_owned(), "json".to_owned()];

        if self.all_approvals {
            args.push("--all-approvals".to_owned());
        }
        if self.all_reviewers {
            args.push("--all-reviewers".to_owned());
        }
        if self.comments {
            args.push("--comments".to_owned());
        }
        if self.commit_message {
            args.push("--commit-message".to_owned());
        }
        if self.current_patch_set {
            args.push("--current-patch-set".to_owned());
        }
        if self.dependencies {
            args.push("--dependencies".to_owned());
        }
        if self.files {
            args.push("--files".to_owned());
        }
        if self.no_limit {
            args.push("--no-limit".to_owned());
        }
        if self.patch_sets {
            args.push("--patch-sets".to_owned());
        }
        if self.start > 0 {
            args.push("--start".to_owned());
            args.push(self.start.to_string())
        }
        if self.submit_records {
            args.push("--submit-records".to_owned());
        }

        args.push("--".to_owned());
        args.push(self.query.to_string());

        args
    }

    /// Include information about all patch sets and approvals.
    #[allow(dead_code)]
    pub fn all_approvals(mut self) -> Self {
        self.all_approvals = true;
        self
    }

    /// Include all reviewers.
    #[allow(dead_code)]
    pub fn all_reviewers(mut self) -> Self {
        self.all_reviewers = true;
        self
    }

    /// Include patch set and inline comments.
    #[allow(dead_code)]
    pub fn comments(mut self) -> Self {
        self.comments = true;
        self
    }

    /// Include the full commit message for a change.
    #[allow(dead_code)]
    pub fn commit_message(mut self) -> Self {
        self.commit_message = true;
        self
    }

    /// Include information about current patch set.
    #[allow(dead_code)]
    pub fn current_patch_set(mut self) -> Self {
        self.current_patch_set = true;
        self
    }

    /// Include depends-on and needed-by information.
    #[allow(dead_code)]
    pub fn dependencies(mut self) -> Self {
        self.dependencies = true;
        self
    }

    /// Include file list on patch sets.
    #[allow(dead_code)]
    pub fn files(mut self) -> Self {
        self.files = true;
        self
    }

    /// Return all results, overriding the default limit.
    #[allow(dead_code)]
    pub fn no_limit(mut self) -> Self {
        self.no_limit = true;
        self
    }

    /// Include information about all patch sets.
    #[allow(dead_code)]
    pub fn patch_sets(mut self) -> Self {
        self.patch_sets = true;
        self
    }

    /// Number of changes to skip.
    #[allow(dead_code)]
    pub fn start(mut self, start: usize) -> Self {
        self.start = start;
        self
    }

    /// Include submit and label status.
    #[allow(dead_code)]
    pub fn submit_records(mut self) -> Self {
        self.submit_records = true;
        self
    }
}

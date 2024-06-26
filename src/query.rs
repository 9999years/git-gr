/// Options for performing a `gerrit query`.
///
/// Not modeled: `--deadline`.
#[derive(Default, Debug, Clone)]
pub struct QueryOptions {
    /// The query to execute.
    query: String,
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

impl QueryOptions {
    /// Construct query options wrapping the given string.
    pub fn new(query: String) -> Self {
        Self {
            query,
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

    /// Get the query string.
    pub fn query_string(&self) -> &str {
        &self.query
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

use crate::approval::Approval;
use crate::author::Author;

/// The current patch set in a Gerrit change.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CurrentPatchSet {
    /// Patch set number.
    pub number: u64,
    /// Git commit hash.
    pub revision: String,
    /// Parent Git commit hashes.
    pub parents: Vec<String>,
    /// Git ref name.
    #[serde(rename = "ref")]
    pub ref_name: String,
    /// The change's uploader.
    pub uploader: Author,
    /// The change's author.
    pub author: Author,
    /// Created timestamp.
    ///
    /// Unix epoch.
    created_on: u64,
    /// Patch kind, e.g. `TRIVIAL_REBASE`.
    kind: String,
    /// The approvals for this patchset.
    #[serde(default)]
    pub approvals: Vec<Approval>,
    /// The number of inserted lines in the patchset.
    pub size_insertions: u64,
    /// The number of deleted lines in the patchset.
    pub size_deletions: u64,
}

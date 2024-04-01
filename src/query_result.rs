use miette::IntoDiagnostic;
use serde::de::DeserializeOwned;

use crate::author::Author;
use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;
use crate::current_patch_set::CurrentPatchSet;

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult<T> {
    pub changes: Vec<T>,
    pub stats: Option<QueryStatistics>,
}

impl<T> QueryResult<T>
where
    T: DeserializeOwned,
{
    pub fn from_stdout(stdout: &str) -> miette::Result<Self> {
        let mut ret = Self {
            changes: Vec::new(),
            stats: None,
        };

        for line in stdout.lines() {
            match serde_json::from_str::<QueryRow<T>>(&line).into_diagnostic()? {
                QueryRow::Change(change) => {
                    ret.changes.push(change);
                }
                QueryRow::Stats(stats) => {
                    ret.stats = Some(stats);
                }
            }
        }

        Ok(ret)
    }
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum QueryRow<T> {
    Change(T),
    Stats(QueryStatistics),
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryStatistics {
    row_count: usize,
    more_changes: bool,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    project: String,
    branch: String,
    id: ChangeId,
    number: ChangeNumber,
    owner: Author,
    url: String,
    hashtags: Vec<String>,
    created_on: u64,
    last_updated: u64,
    open: bool,
    status: String,
}

/// A [`Change`] with a [`CurrentPatchSet`].
///
/// TODO: Make this generic over the mixin type?
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeCurrentPatchSet {
    #[serde(flatten)]
    pub change: Change,
    pub current_patch_set: CurrentPatchSet,
}

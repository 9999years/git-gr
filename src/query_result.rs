use miette::IntoDiagnostic;
use serde::de::DeserializeOwned;

use crate::author::Author;
use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;
use crate::current_patch_set::CurrentPatchSet;
use crate::depends_on::DependsOn;
use crate::needed_by::NeededBy;

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
            let row = serde_json::from_str::<serde_json::Value>(line).into_diagnostic()?;
            // Awful! Truly rancid!
            let is_stats = row
                .as_object()
                .and_then(|object| object.get("type"))
                .and_then(|type_value| type_value.as_str())
                .map(|stats_value| stats_value == "stats")
                .unwrap_or(false);

            if is_stats {
                ret.stats = Some(serde_json::from_value::<QueryStatistics>(row).into_diagnostic()?);
            } else {
                ret.changes
                    .push(serde_json::from_value::<T>(row).into_diagnostic()?);
            }
        }

        Ok(ret)
    }
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
    pub project: String,
    pub branch: String,
    pub id: ChangeId,
    pub number: ChangeNumber,
    pub subject: Option<String>,
    pub owner: Author,
    pub url: String,
    pub hashtags: Vec<String>,
    created_on: u64,
    last_updated: u64,
    pub open: bool,
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

/// A [`Change`] with a [`DependsOn`] and [`NeededBy`].
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDependencies {
    #[serde(flatten)]
    pub change: Change,
    pub depends_on: Vec<DependsOn>,
    pub needed_by: Vec<NeededBy>,
}

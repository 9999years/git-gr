use std::collections::BTreeSet;

use comfy_table::Cell;
use comfy_table::Color;
use miette::IntoDiagnostic;
use serde::de::DeserializeOwned;

use crate::change::Change;
use crate::change_number::ChangeNumber;
use crate::change_status::ChangeStatus;
use crate::current_patch_set::CurrentPatchSet;
use crate::depends_on::DependsOn;
use crate::gerrit::Gerrit;
use crate::needed_by::NeededBy;
use crate::submit_records::SubmitRecord;
use crate::submit_status::SubmitStatus;

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
#[allow(dead_code)]
pub struct QueryStatistics {
    row_count: usize,
    more_changes: bool,
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

/// A [`Change`] with [`SubmitRecord`]s.
///
/// TODO: Make this generic over the mixin type?
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSubmitRecords {
    #[serde(flatten)]
    pub change: Change,
    pub submit_records: Vec<SubmitRecord>,
}

impl ChangeSubmitRecords {
    pub fn ready_cell(&self) -> Cell {
        match self.submit_records.first() {
            Some(record) => match record.status {
                SubmitStatus::Ok => Cell::new("✔").fg(Color::Green),
                SubmitStatus::NotReady => Cell::new("✗").fg(Color::Red),
                SubmitStatus::Closed => {
                    Cell::new("closed").add_attribute(comfy_table::Attribute::Dim)
                }
                SubmitStatus::RuleError => Cell::new("error").fg(Color::Red),
            },
            None => Cell::new(""),
        }
    }
}

/// A [`Change`] with a [`DependsOn`] and [`NeededBy`].
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDependencies {
    #[serde(flatten)]
    pub change: Change,
    #[serde(default)]
    pub depends_on: Vec<DependsOn>,
    #[serde(default)]
    pub needed_by: Vec<NeededBy>,
}

impl ChangeDependencies {
    /// Remove merged and abandoned dependencies from this set.
    pub fn filter_unmerged(mut self, gerrit: &Gerrit) -> miette::Result<Self> {
        let depends_on = std::mem::take(&mut self.depends_on);

        for dependency in depends_on {
            let change = gerrit.get_change(dependency.number)?;
            if let ChangeStatus::New = change.status {
                self.depends_on.push(dependency);
            }
        }

        let needed_by = std::mem::take(&mut self.needed_by);

        for dependency in needed_by {
            let change = gerrit.get_change(dependency.number)?;
            if let ChangeStatus::New = change.status {
                self.needed_by.push(dependency);
            }
        }

        Ok(self)
    }

    /// Get the change numbers this change depends on.
    ///
    /// These are deduplicated by change number.
    pub fn depends_on_numbers(&self) -> BTreeSet<ChangeNumber> {
        self.depends_on
            .iter()
            .map(|depends_on| depends_on.number)
            .collect()
    }

    /// Get the change numbers this change is needed by.
    ///
    /// These are deduplicated by change number.
    pub fn needed_by_numbers(&self) -> BTreeSet<ChangeNumber> {
        self.needed_by
            .iter()
            .map(|needed_by| needed_by.number)
            .collect()
    }
}

use std::collections::BTreeSet;

use comfy_table::Attribute;
use comfy_table::Cell;
use comfy_table::Color;
use miette::IntoDiagnostic;
use serde_with::serde_as;
use serde_with::TimestampSeconds;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::author::Author;
use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;
use crate::change_status::ChangeStatus;
use crate::current_patch_set::CurrentPatchSet;
use crate::depends_on::DependsOn;
use crate::gerrit::Gerrit;
use crate::needed_by::NeededBy;
use crate::patchset::ChangePatchset;
use crate::patchset::Patchset;
use crate::submit_records::SubmitRecord;
use crate::submit_status::SubmitStatus;

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
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
    #[allow(dead_code)]
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub created_on: OffsetDateTime,
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub last_updated: OffsetDateTime,
    pub open: bool,
    pub status: ChangeStatus,
    #[serde(default)]
    pub wip: bool,
    pub current_patch_set: CurrentPatchSet,
    pub submit_records: Vec<SubmitRecord>,
    #[serde(default)]
    pub depends_on: Vec<DependsOn>,
    #[serde(default)]
    pub needed_by: Vec<NeededBy>,
}

impl Change {
    pub fn patchset(&self) -> ChangePatchset {
        ChangePatchset {
            change: self.number,
            patchset: Patchset::new(self.current_patch_set.number),
        }
    }

    pub fn status_cell(&self) -> Cell {
        match self.status {
            ChangeStatus::Merged => Cell::new("merged").fg(Color::Magenta),
            ChangeStatus::Abandoned => Cell::new("closed").fg(Color::Red),
            ChangeStatus::New => {
                if self.wip {
                    Cell::new("wip").add_attribute(Attribute::Dim)
                } else {
                    Cell::new("open").fg(Color::Green)
                }
            }
        }
    }

    pub fn last_updated_cell(&self, timestamp_format: TimestampFormat) -> miette::Result<Cell> {
        // _Something_ in my transitive dependency tree is making threads, which makes
        // `time` refuse to get the local timezone. I don't think anything is going to
        // change the local timezone so it's PRobably Fine.
        //
        // Safety: It's fine. It's FINE.
        unsafe {
            time::util::local_offset::set_soundness(time::util::local_offset::Soundness::Unsound)
        };
        let now = OffsetDateTime::now_local().into_diagnostic()?;
        let now_date = now.date();
        let date = self.last_updated.date();
        let formatted = {
            if now_date == date {
                // Today.
                let format = match timestamp_format {
                    TimestampFormat::TwelveHour => {
                        format_description!(
                            "[hour padding:none repr:12]:[minute] [period case:lower]"
                        )
                    }
                    TimestampFormat::TwentyFourHour => {
                        format_description!("[hour padding:none repr:24]:[minute]")
                    }
                };
                self.last_updated.format(format)
            } else if now_date.year() == date.year() {
                self.last_updated
                    .format(format_description!("[month]-[day]"))
            } else {
                self.last_updated
                    .format(format_description!("[year]-[month]-[day]"))
            }
        }
        .into_diagnostic()?;
        Ok(Cell::new(formatted))
    }

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

/// Support for Europeans.
#[derive(Debug, Clone, Copy)]
pub enum TimestampFormat {
    /// 12-hour time.
    TwelveHour,
    /// 24-hour time.
    TwentyFourHour,
}

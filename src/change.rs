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

#[serde_as]
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
    #[allow(dead_code)]
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub created_on: OffsetDateTime,
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub last_updated: OffsetDateTime,
    pub open: bool,
    pub status: ChangeStatus,
    #[serde(default)]
    pub wip: bool,
}

impl Change {
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
}

/// Support for Europeans.
#[derive(Debug, Clone, Copy)]
pub enum TimestampFormat {
    /// 12-hour time.
    TwelveHour,
    /// 24-hour time.
    TwentyFourHour,
}

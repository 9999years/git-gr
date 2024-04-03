use comfy_table::Attribute;
use comfy_table::Cell;
use comfy_table::Color;

use crate::author::Author;
use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;
use crate::change_status::ChangeStatus;

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
    created_on: u64,
    #[allow(dead_code)]
    last_updated: u64,
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
}

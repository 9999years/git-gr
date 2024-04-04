use std::collections::BTreeSet;

use crate::change_number::ChangeNumber;
use crate::related_change_and_commit_info::RelatedChangeAndCommitInfo;

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RelatedChangesInfo {
    pub changes: Vec<RelatedChangeAndCommitInfo>,
}

impl RelatedChangesInfo {
    pub fn change_numbers(&self) -> BTreeSet<ChangeNumber> {
        self.changes
            .iter()
            .flat_map(|change| change.change_number)
            .collect()
    }
}

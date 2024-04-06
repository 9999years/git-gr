use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;
use crate::change_status::ChangeStatus;
use crate::commit_info::CommitInfo;
use crate::patchset::Patchset;

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct RelatedChangeAndCommitInfo {
    pub project: String,
    pub change_id: Option<ChangeId>,
    pub commit: CommitInfo,
    #[serde(rename = "_change_number")]
    pub change_number: Option<ChangeNumber>,
    #[serde(rename = "_revision_number")]
    pub revision_number: Option<Patchset>,
    #[serde(rename = "_current_revision_number")]
    pub current_revision_number: Option<Patchset>,
    pub status: Option<ChangeStatus>,
    pub submittable: bool,
}

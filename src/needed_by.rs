use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;

/// A change that the currrent change is needed by.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NeededBy {
    /// Change ID.
    pub id: ChangeId,
    /// Change number.
    pub number: ChangeNumber,
    /// Git commit hash.
    pub revision: String,
    #[serde(default)]
    pub is_current_patch_set: bool,
}

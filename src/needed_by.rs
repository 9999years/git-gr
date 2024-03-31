use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;

/// A change that the currrent change is needed by.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeededBy {
    /// Change ID.
    id: ChangeId,
    /// Change number.
    number: ChangeNumber,
    /// Git commit hash.
    revision: String,
    #[serde(default)]
    is_current_patch_set: bool,
}

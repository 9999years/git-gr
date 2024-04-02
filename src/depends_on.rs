use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;

/// A change that the current change depends on.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DependsOn {
    /// Change ID.
    id: ChangeId,
    /// Change number.
    number: ChangeNumber,
    /// Git commit hash.
    revision: String,
    #[serde(default)]
    is_current_patch_set: bool,
}

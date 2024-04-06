use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;
use crate::commit_hash::CommitHash;

/// A change that the currrent change is needed by.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NeededBy {
    /// Change ID.
    pub id: ChangeId,
    /// Change number.
    pub number: ChangeNumber,
    /// Git commit hash.
    pub revision: CommitHash,
    #[serde(default)]
    pub is_current_patch_set: bool,
}

/// A change which is needed by another change.
///
/// This allows constructing a graph of changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NeededByRelation {
    pub change: ChangeNumber,
    pub needed_by: ChangeNumber,
}

use crate::author::Author;
use crate::change::Change;
use crate::change_id::ChangeId;
use crate::change_status::ChangeStatus;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
/// Information about a change in a dependency graph.
pub struct ChangeMetadata {
    /// Is the change a work-in-progress?
    wip: bool,
    /// Is the change open or abandoned?
    status: ChangeStatus,
    /// The change's author.
    owner: Author,
    /// The change's ID.
    id: ChangeId,
}

impl ChangeMetadata {
    pub fn new(change: &Change) -> Self {
        Self {
            wip: change.wip,
            status: change.status,
            owner: change.owner.clone(),
            id: change.id.clone(),
        }
    }
}

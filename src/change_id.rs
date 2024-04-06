use derive_more::{AsRef, Constructor, Deref, DerefMut, Display, From, Into};

use crate::change::Change;

/// A Gerrit change ID.
///
/// This is a string starting with `I` and followed by 40 hex characters.
#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Display,
    Into,
    From,
    AsRef,
    Deref,
    DerefMut,
    Constructor,
)]
#[serde(transparent)]
pub struct ChangeId(String);

impl From<Change> for ChangeId {
    fn from(change: Change) -> Self {
        change.id
    }
}

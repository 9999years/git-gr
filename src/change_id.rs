use std::fmt::Display;
use std::ops::Deref;

/// A Gerrit change ID.
///
/// This is a string starting with `I` and followed by 40 hex characters.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct ChangeId(pub String);

impl Display for ChangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for ChangeId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

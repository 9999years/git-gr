use std::fmt::Display;
use std::ops::Deref;

/// A Gerrit change ID.
///
/// This is a string starting with `I` and followed by like 40 hex characters, IDK I'm offline RN.
///
/// TODO: Represent as a big number...?
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

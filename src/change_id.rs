use std::fmt::Display;

use serde::Deserialize;

/// A Gerrit change ID.
///
/// This is a string starting with `I` and followed by like 40 hex characters, IDK I'm offline RN.
///
/// TODO: Represent as a big number...?
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChangeId(String);

impl Display for ChangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for ChangeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Self)
    }
}

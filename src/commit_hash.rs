use std::fmt::Display;
use std::ops::Deref;

/// A Git commit hash.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct CommitHash(pub String);

impl CommitHash {
    /// Get an abbreviated 8-character Git hash.
    pub fn abbrev(&self) -> &str {
        &self.0[..8]
    }
}

impl Display for CommitHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for CommitHash {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

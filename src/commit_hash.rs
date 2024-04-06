use derive_more::{AsRef, Constructor, Deref, DerefMut, Display, From, Into};

/// A Git commit hash.
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
pub struct CommitHash(String);

impl CommitHash {
    /// Get an abbreviated 8-character Git hash.
    pub fn abbrev(&self) -> &str {
        &self.0[..8]
    }
}

use std::fmt::Display;

use clap::builder::RangedU64ValueParser;
use clap::builder::TypedValueParser;
use clap::builder::ValueParserFactory;
use derive_more::{AsRef, Constructor, Deref, DerefMut, Display, From, Into};

use crate::change::Change;
use crate::change_number::ChangeNumber;

#[derive(
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
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
pub struct Patchset(u64);

#[derive(Clone)]
pub struct PatchsetParser;

impl ValueParserFactory for Patchset {
    type Parser = PatchsetParser;

    fn value_parser() -> Self::Parser {
        PatchsetParser
    }
}

impl TypedValueParser for PatchsetParser {
    type Value = Patchset;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        RangedU64ValueParser::new()
            .parse_ref(cmd, arg, value)
            .map(Patchset)
    }
}

/// A [`ChangeNumber`] and a [`Patchset`].
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct ChangePatchset {
    pub change: ChangeNumber,
    pub patchset: Patchset,
}

impl From<Change> for ChangePatchset {
    fn from(change: Change) -> Self {
        change.patchset()
    }
}

impl ChangePatchset {
    pub fn git_ref(&self) -> String {
        format!(
            "refs/changes/{}/{}/{}",
            self.change.last_two(),
            self.change,
            self.patchset
        )
    }
}

impl Display for ChangePatchset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.change, self.patchset)
    }
}

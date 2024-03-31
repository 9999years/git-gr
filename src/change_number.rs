use std::fmt::Display;

use clap::builder::TypedValueParser;
use clap::builder::ValueParser;
use clap::builder::ValueParserFactory;
use serde::Deserialize;

/// A Gerrit change number.
///
/// Unlike a change ID, this is a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChangeNumber(u64);

impl Display for ChangeNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ChangeNumber {
    pub fn last_two(&self) -> String {
        let str = self.to_string();
        let len = str.len();
        if len <= 2 {
            str
        } else {
            // TODO: uhhhh
            str[len - 2..].to_owned()
        }
    }

    pub fn git_ref(&self, patch_number: u32) -> String {
        format!("refs/changes/{}/{}/{patch_number}", self.last_two(), self)
    }
}

impl<'de> Deserialize<'de> for ChangeNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        u64::deserialize(deserializer).map(Self)
    }
}

#[derive(Clone)]
pub struct ChangeNumberParser;

impl ValueParserFactory for ChangeNumber {
    type Parser = ChangeNumberParser;

    fn value_parser() -> Self::Parser {
        ChangeNumberParser
    }
}

impl TypedValueParser for ChangeNumberParser {
    type Value = ChangeNumber;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        todo!()
    }
}

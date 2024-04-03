use std::fmt::Display;

use clap::builder::RangedU64ValueParser;
use clap::builder::TypedValueParser;
use clap::builder::ValueParserFactory;

use crate::gerrit::Gerrit;

/// A Gerrit change number.
///
/// Unlike a change ID, this is a number.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(transparent)]
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

    pub fn pretty(&self, gerrit: &Gerrit) -> miette::Result<String> {
        Ok(match gerrit.get_change(*self)?.subject {
            Some(subject) => format!("{} ({})", self, subject),
            None => self.to_string(),
        })
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
        RangedU64ValueParser::new()
            .parse_ref(cmd, arg, value)
            .map(ChangeNumber)
    }
}

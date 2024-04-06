use std::fmt::Display;

use clap::builder::RangedU64ValueParser;
use clap::builder::TypedValueParser;
use clap::builder::ValueParserFactory;
use owo_colors::OwoColorize;
use owo_colors::Stream::Stderr;
use owo_colors::Style;

use crate::change::Change;
use crate::gerrit::Gerrit;
use crate::patchset::ChangePatchset;
use crate::patchset::Patchset;

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

    pub fn with_patchset(&self, patchset: Patchset) -> ChangePatchset {
        ChangePatchset {
            change: *self,
            patchset,
        }
    }

    pub fn pretty(&self, gerrit: &Gerrit) -> miette::Result<String> {
        let subject = gerrit.get_change(*self)?.subject;
        Ok(format!(
            "{}{}",
            self.if_supports_color(Stderr, |change| Style::new().bold().green().style(change)),
            subject
                .map(|subject| format!(" ({subject})"))
                .unwrap_or_default()
                .if_supports_color(Stderr, |subject| Style::new().dimmed().style(subject))
        ))
    }
}

impl From<Change> for ChangeNumber {
    fn from(change: Change) -> Self {
        change.number
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

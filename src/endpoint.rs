use clap::builder::StringValueParser;
use clap::builder::TypedValueParser;
use clap::builder::ValueParserFactory;
use derive_more::{AsRef, Deref, DerefMut, Display, Into};

/// An API endpoint, with no leading `/`.
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
    AsRef,
    Deref,
    DerefMut,
)]
#[serde(transparent)]
pub struct Endpoint(String);

impl Endpoint {
    pub fn new(endpoint: &str) -> Self {
        Self(endpoint.trim_start_matches('/').to_owned())
    }
}

#[derive(Clone)]
pub struct EndpointParser;

impl ValueParserFactory for Endpoint {
    type Parser = EndpointParser;

    fn value_parser() -> Self::Parser {
        EndpointParser
    }
}

impl TypedValueParser for EndpointParser {
    type Value = Endpoint;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        StringValueParser::new()
            .parse_ref(cmd, arg, value)
            .map(|value| Endpoint::new(&value))
    }
}

use std::fmt::Display;

use crate::endpoint::Endpoint;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GerritHost {
    pub username: String,
    pub host: String,
    pub port: u16,
}

impl GerritHost {
    /// The `ssh` destination to connect to.
    pub fn connect_to(&self) -> String {
        format!("ssh://{}@{}:{}", self.username, self.host, self.port)
    }

    /// Given an endpoint path, format an HTTP request URL.
    pub fn endpoint(&self, endpoint: &Endpoint) -> String {
        format!("https://{}/a/{endpoint}", self.host)
    }
}

impl Display for GerritHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.connect_to())
    }
}

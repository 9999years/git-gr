use std::fmt::Display;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubmitStatus {
    Ok,
    NotReady,
    Closed,
    RuleError,
}

impl Display for SubmitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmitStatus::Ok => write!(f, "ready"),
            SubmitStatus::NotReady => write!(f, "not ready"),
            SubmitStatus::Closed => write!(f, "closed"),
            SubmitStatus::RuleError => write!(f, "rule error"),
        }
    }
}

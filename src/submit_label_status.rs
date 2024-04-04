use std::fmt::Display;

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubmitLabelStatus {
    Ok,
    Reject,
    May,
    Need,
    Impossible,
}

impl Display for SubmitLabelStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmitLabelStatus::May => write!(f, "optional"),
            SubmitLabelStatus::Need => write!(f, "needed"),
            SubmitLabelStatus::Ok => write!(f, "approved"),
            SubmitLabelStatus::Reject => write!(f, "blocking"),
            SubmitLabelStatus::Impossible => write!(f, "impossible"),
        }
    }
}

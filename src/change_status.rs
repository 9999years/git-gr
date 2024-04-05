use std::fmt::Display;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChangeStatus {
    New,
    Merged,
    Abandoned,
}

impl Display for ChangeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeStatus::New => write!(f, "new"),
            ChangeStatus::Merged => write!(f, "merged"),
            ChangeStatus::Abandoned => write!(f, "abandoned"),
        }
    }
}

use crate::author::Author;
use crate::submit_label_status::SubmitLabelStatus;

/// A submission label in a Gerrit change.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct SubmitLabel {
    label: String,
    by: Option<Author>,
    status: SubmitLabelStatus,
}

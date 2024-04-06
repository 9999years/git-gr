use crate::submit_label::SubmitLabel;
use crate::submit_status::SubmitStatus;

/// A submission record in a Gerrit change.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct SubmitRecord {
    pub status: SubmitStatus,
    labels: Vec<SubmitLabel>,
}

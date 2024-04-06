use crate::submit_label::SubmitLabel;
use crate::submit_status::SubmitStatus;

/// A submission record in a Gerrit change.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct SubmitRecord {
    pub status: SubmitStatus,
    #[serde(default)]
    labels: Vec<SubmitLabel>,
}

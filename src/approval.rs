use crate::author::Author;

/// The current patch set in a Gerrit change.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Approval {
    /// The approval type, like `Verified`.
    #[serde(rename = "type")]
    type_: String,
    /// The approval description.
    description: Option<String>,
    /// The value.
    ///
    /// Generally(?) a number like `-1` or `+2`.
    value: String,
    by: Author,
}

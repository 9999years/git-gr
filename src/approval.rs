use crate::author::User;

/// The current patch set in a Gerrit change.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    /// The approval type, like `Verified`.
    #[serde(rename = "type")]
    type_: String,
    /// The approval description.
    description: String,
    /// The value.
    ///
    /// Generally(?) a number like `-1` or `+2`.
    value: String,
    by: User,
}

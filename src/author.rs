/// An author of a Gerrit change.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Author {
    pub email: Option<String>,
    pub name: String,
    pub username: String,
}

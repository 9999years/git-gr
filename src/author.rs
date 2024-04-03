/// An author of a Gerrit change.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Author {
    email: String,
    #[serde(flatten)]
    user: User,
}

/// A Gerrit user.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct User {
    name: String,
    username: String,
}

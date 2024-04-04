use crate::git_person_info::GitPersonInfo;

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CommitInfo {
    commit: Option<String>,
    parents: Vec<CommitInfoMinimal>,
    author: GitPersonInfo,
    subject: String,
    message: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CommitInfoMinimal {
    commit: String,
    subject: Option<String>,
}

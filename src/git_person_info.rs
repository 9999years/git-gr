#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct GitPersonInfo {
    pub name: String,
    pub email: String,
    date: String,
    tz: i16,
}

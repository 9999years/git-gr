use miette::IntoDiagnostic;
use serde::de::DeserializeOwned;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult<T> {
    pub changes: Vec<T>,
    pub stats: Option<QueryStatistics>,
}

impl<T> QueryResult<T>
where
    T: DeserializeOwned,
{
    pub fn from_stdout(stdout: &str) -> miette::Result<Self> {
        let mut ret = Self {
            changes: Vec::new(),
            stats: None,
        };

        for line in stdout.lines() {
            let row = serde_json::from_str::<serde_json::Value>(line).into_diagnostic()?;
            // Awful! Truly rancid!
            let is_stats = row
                .as_object()
                .and_then(|object| object.get("type"))
                .and_then(|type_value| type_value.as_str())
                .map(|stats_value| stats_value == "stats")
                .unwrap_or(false);

            if is_stats {
                ret.stats = Some(serde_json::from_value::<QueryStatistics>(row).into_diagnostic()?);
            } else {
                ret.changes
                    .push(serde_json::from_value::<T>(row).into_diagnostic()?);
            }
        }

        Ok(ret)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct QueryStatistics {
    row_count: usize,
    more_changes: bool,
}

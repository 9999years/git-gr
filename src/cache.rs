use std::fmt::Display;

use crate::change::Change;
use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;
use crate::commit_hash::CommitHash;
use crate::endpoint::Endpoint;
use crate::patchset::ChangePatchset;
use crate::query_result::QueryResult;

#[derive(Debug, Clone)]
pub enum CacheKey {
    /// A change request, indexed by number.
    Change(ChangeNumber),
    /// A change request, indexed by change ID.
    ChangeId(ChangeId),
    /// A change request, indexed by an arbitrary query.
    ChangeQuery(String),
    /// A change request, fetched at a given patchset.
    Fetch(ChangePatchset),
    /// A query to the change database.
    Query(String),
    /// A request to the REST API.
    Api(Endpoint),
}

impl Display for CacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheKey::Change(change) => write!(f, "change-{change}"),
            // Safety: Change numbers are not hexadecimal.todo!()
            CacheKey::ChangeId(change) => write!(f, "change-{change}"),
            CacheKey::ChangeQuery(query) => write!(f, "change-query-{query}"),
            CacheKey::Fetch(change) => write!(f, "fetch-{change}"),
            CacheKey::Query(query) => write!(f, "query-{query}"),
            CacheKey::Api(endpoint) => write!(f, "api-{endpoint}"),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum CacheValue {
    Change(Change),
    Fetch(CommitHash),
    Query(QueryResult<Change>),
    Api(String),
}

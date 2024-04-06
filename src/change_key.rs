use derive_more::Display;
use derive_more::From;
use derive_more::TryInto;

use crate::cache::CacheKey;
use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;

/// A key for looking up a change in Gerrit.
///
/// Although the `Id` and `Query` constructors are both strings, the `Id` constructor will be
/// better at hitting the cache.
#[derive(serde::Serialize, serde::Deserialize, Debug, Display, Clone, From, TryInto)]
pub enum ChangeKey {
    Number(ChangeNumber),
    Id(ChangeId),
    Query(String),
}

impl From<ChangeKey> for CacheKey {
    fn from(value: ChangeKey) -> Self {
        match value {
            ChangeKey::Number(change) => CacheKey::Change(change),
            ChangeKey::Id(change) => CacheKey::ChangeId(change),
            ChangeKey::Query(change) => CacheKey::ChangeQuery(change),
        }
    }
}

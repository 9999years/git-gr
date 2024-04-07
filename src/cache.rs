use std::fmt::Display;
use std::time::Duration;

use cached::DiskCache;
use cached::DiskCacheError;
use cached::IOCached;
use miette::Context;
use miette::IntoDiagnostic;

use crate::change::Change;
use crate::change_id::ChangeId;
use crate::change_number::ChangeNumber;
use crate::commit_hash::CommitHash;
use crate::endpoint::Endpoint;
use crate::gerrit_project::GerritProject;
use crate::patchset::ChangePatchset;
use crate::query_result::QueryResult;

const SECONDS_PER_MINUTE: u64 = 60;
pub const CACHE_LIFESPAN: Duration = Duration::from_secs(10 * SECONDS_PER_MINUTE);

/// A Gerrit API cache.
pub enum GerritCache {
    /// It doesn't cache anything!
    None,
    /// It caches to disk.
    Disk(DiskCache<CacheKey, CacheValue>),
}

impl GerritCache {
    pub fn new(host: &GerritProject) -> miette::Result<Self> {
        Ok(Self::Disk(
            DiskCache::new(&host.to_string())
                .set_lifespan(CACHE_LIFESPAN.as_secs())
                .build()
                .into_diagnostic()
                .wrap_err("Failed to initialize Gerrit API cache")?,
        ))
    }

    pub fn clear_cache(&mut self) {
        match self {
            GerritCache::None => todo!(),
            GerritCache::Disk(cache) => {
                // `cached` has no `cache_clear` operation, so we have to do this workaround.
                // See: https://github.com/jaemk/cached/issues/197

                // BUG: `remove_expired_entries` only removes _unexpired_ entries, so we need to set
                // the expiration time to ~infinity for this to work.
                // See: https://github.com/jaemk/cached/pull/198
                let lifespan = cache.cache_set_lifespan(u64::MAX);

                cache.remove_expired_entries();

                match lifespan {
                    Some(lifespan) => {
                        cache.cache_set_lifespan(lifespan);
                    }
                    None => {
                        cache.cache_set_lifespan(CACHE_LIFESPAN.as_secs());
                    }
                }
            }
        }
    }

    /// Construct the disk cache to replace it after calling [`Self::deattach_cache`].
    ///
    /// Returns the old cache.
    pub fn attach_cache(&mut self, host: &GerritProject) -> miette::Result<Self> {
        Ok(std::mem::replace(self, Self::new(host)?))
    }

    /// Destruct the disk cache to e.g. allow another program to access it.
    ///
    /// Returns the old cache.
    pub fn deattach_cache(&mut self) -> Self {
        std::mem::replace(self, Self::None)
    }
}

impl IOCached<CacheKey, CacheValue> for GerritCache {
    type Error = DiskCacheError;

    fn cache_get(&self, k: &CacheKey) -> Result<Option<CacheValue>, Self::Error> {
        match self {
            GerritCache::None => Ok(None),
            GerritCache::Disk(cache) => cache.cache_get(k),
        }
    }

    fn cache_set(&self, k: CacheKey, v: CacheValue) -> Result<Option<CacheValue>, Self::Error> {
        match self {
            GerritCache::None => Ok(None),
            GerritCache::Disk(cache) => cache.cache_set(k, v),
        }
    }

    fn cache_remove(&self, k: &CacheKey) -> Result<Option<CacheValue>, Self::Error> {
        match self {
            GerritCache::None => Ok(None),
            GerritCache::Disk(cache) => cache.cache_remove(k),
        }
    }

    fn cache_set_refresh(&mut self, refresh: bool) -> bool {
        match self {
            GerritCache::None => false,
            GerritCache::Disk(cache) => cache.cache_set_refresh(refresh),
        }
    }
}

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
    Change(Box<Change>),
    Fetch(CommitHash),
    Query(QueryResult<Change>),
    Api(String),
}

//! In-memory cache of rendered sites, keyed by subdomain and validated by
//! publish id. Every request still performs the one indexed resolver read —
//! that is what makes a republish (or unpublish) visible immediately — but
//! rendering is done once per publish, not per request: a cache entry is a
//! hit only while its publish id matches what the resolver just returned,
//! so serving stale content is impossible by construction.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use alo_store::SitePublishId;

use super::rendered::RenderedSite;

/// Upper bound on cached sites; above it an arbitrary entry is evicted.
/// Rendering is cheap (a few ms) so eviction quality is not worth an LRU
/// until real traffic says otherwise.
const MAX_SITES: usize = 512;

/// The publish-keyed render cache. Lock poisoning is deliberately ignored:
/// a rendered site is plain data, valid regardless of another thread's
/// panic, and the public service must keep serving.
#[derive(Default)]
pub struct SiteCache {
    inner: RwLock<HashMap<String, Arc<RenderedSite>>>,
}

impl SiteCache {
    /// The cached render of `subdomain`, only if it is exactly `publish`.
    pub fn get(&self, subdomain: &str, publish: &SitePublishId) -> Option<Arc<RenderedSite>> {
        let map = self.inner.read().unwrap_or_else(PoisonError::into_inner);
        map.get(subdomain)
            .filter(|site| site.publish == *publish)
            .cloned()
    }

    /// Stores a fresh render, replacing any earlier publish of the same
    /// subdomain and evicting an arbitrary entry at the bound.
    pub fn put(&self, subdomain: &str, site: Arc<RenderedSite>) {
        let mut map = self.inner.write().unwrap_or_else(PoisonError::into_inner);
        if !map.contains_key(subdomain)
            && map.len() >= MAX_SITES
            && let Some(evict) = map.keys().next().cloned()
        {
            map.remove(&evict);
        }
        map.insert(subdomain.to_owned(), site);
    }
}

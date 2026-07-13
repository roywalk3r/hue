use bytes::Bytes;
use moka::future::Cache;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct SegmentKey {
    pub url: String,
    pub range: Option<String>,
}

#[derive(Clone)]
pub struct CachedSegment {
    pub body: Bytes,
    pub content_type: String,
    pub content_length: Option<u64>,
    pub accept_ranges: bool,
    pub cache_control: &'static str,
}

pub struct SegmentCache {
    inner: Cache<SegmentKey, CachedSegment>,
}

impl SegmentCache {
    pub fn new(max_bytes: u64) -> Self {
        let inner = Cache::builder()
            .max_capacity(max_bytes)
            .weigher(|_k, v: &CachedSegment| v.body.len() as u32)
            .build();
        Self { inner }
    }

    pub async fn get(&self, key: &SegmentKey) -> Option<CachedSegment> {
        self.inner.get(key).await
    }

    pub async fn put(&self, key: SegmentKey, segment: CachedSegment) {
        self.inner.insert(key, segment).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(body: &[u8]) -> CachedSegment {
        CachedSegment {
            body: Bytes::copy_from_slice(body),
            content_type: "video/mp2t".to_string(),
            content_length: Some(body.len() as u64),
            accept_ranges: true,
            cache_control: "public, max-age=5",
        }
    }

    #[tokio::test]
    async fn put_then_get_roundtrip() {
        let cache = SegmentCache::new(1024 * 1024);
        let key = SegmentKey {
            url: "https://example.com/seg1.ts".to_string(),
            range: None,
        };
        let body = segment(b"hello world");

        cache.put(key.clone(), body.clone()).await;
        let got = cache.get(&key).await.expect("entry should be present");
        assert_eq!(got.body, body.body);
        assert_eq!(got.content_type, "video/mp2t");
        assert_eq!(got.content_length, Some(11));
        assert!(got.accept_ranges);
    }

    #[tokio::test]
    async fn different_keys_do_not_collide() {
        let cache = SegmentCache::new(1024 * 1024);
        let key_a = SegmentKey {
            url: "https://example.com/a.ts".to_string(),
            range: None,
        };
        let key_b = SegmentKey {
            url: "https://example.com/b.ts".to_string(),
            range: None,
        };
        let key_c = SegmentKey {
            url: "https://example.com/a.ts".to_string(),
            range: Some("bytes=0-1023".to_string()),
        };

        cache.put(key_a.clone(), segment(b"AAA")).await;
        cache.put(key_b.clone(), segment(b"BBB")).await;
        cache.put(key_c.clone(), segment(b"CCC")).await;

        assert_eq!(cache.get(&key_a).await.unwrap().body, "AAA");
        assert_eq!(cache.get(&key_b).await.unwrap().body, "BBB");
        assert_eq!(cache.get(&key_c).await.unwrap().body, "CCC");
    }

    #[tokio::test]
    async fn weigher_evicts_oldest_when_cap_exceeded() {
        let cap = 100u64;
        let cache = SegmentCache::new(cap);

        for i in 0..6 {
            let key = SegmentKey {
                url: format!("https://example.com/seg{}.ts", i),
                range: None,
            };
            cache.put(key, segment(&vec![0u8; 40])).await;
        }

        cache.inner.run_pending_tasks().await;

        let mut total = 0usize;
        let mut kept_keys = 0usize;
        for i in 0..6 {
            let key = SegmentKey {
                url: format!("https://example.com/seg{}.ts", i),
                range: None,
            };
            if let Some(entry) = cache.get(&key).await {
                total += entry.body.len();
                kept_keys += 1;
            }
        }

        assert!(total as u64 <= cap, "total bytes {} exceeds cap {}", total, cap);
        assert!(kept_keys < 6, "expected oldest entries to be evicted");
    }
}

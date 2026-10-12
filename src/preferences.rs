//! Bounded frequency cache. Independent of dictionaries, models
//! and input hosts. Stores salted fingerprints and counts, not text records.
//! This is not encryption: preferences and their snapshots remain sensitive.
pub mod persistence;
#[cfg(target_os = "linux")]
pub mod store;
#[cfg(target_os = "linux")]
pub mod worker;

use ring::{
    hmac,
    rand::{SecureRandom, SystemRandom},
};
use std::collections::{HashMap, hash_map::RandomState};
use std::hash::{BuildHasher, Hash};
use std::time::{Duration, Instant};

pub const MAX_ENTRIES: usize = 2048;
pub const MAX_KEY_BYTES: usize = 1024;
pub const HALF_LIFE: Duration = Duration::from_secs(24 * 60 * 60);
pub const MAX_FREQUENCY: f32 = 8.0;

struct FingerprintSecret {
    bytes: [u8; 32],
    key: hmac::Key,
}

impl FingerprintSecret {
    fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            key: hmac::Key::new(hmac::HMAC_SHA256, &bytes),
            bytes,
        }
    }

    fn generate() -> Option<Self> {
        let mut bytes = [0; 32];
        SystemRandom::new().fill(&mut bytes).ok()?;
        Some(Self::from_bytes(bytes))
    }

    fn fingerprint(&self, parts: &[&str]) -> PreferenceKey {
        let mut context = hmac::Context::with_key(&self.key);
        context.update(b"suzaku-preference-key-v1\0");
        for part in parts {
            // The key() bounds make each byte length representable as u32.
            context.update(&(part.len() as u32).to_be_bytes());
            context.update(part.as_bytes());
        }
        let tag = context.sign();
        let bytes = tag.as_ref();
        PreferenceKey([
            u64::from_be_bytes(bytes[..8].try_into().unwrap()),
            u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
        ])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PreferenceKey([u64; 2]);

#[derive(Clone, Copy)]
struct Entry {
    frequency: f32,
    updated: Instant,
    revision: u64,
}

impl Entry {
    fn frequency_at(self, now: Instant) -> f32 {
        self.frequency
            * (-now.saturating_duration_since(self.updated).as_secs_f32() / HALF_LIFE.as_secs_f32())
                .exp2()
    }
}

/// An undo receipt is valid only for the exact write it describes. A clear,
/// eviction or later write must never be undone by an old receipt.
pub struct Feedback {
    owner: PreferenceKey,
    key: PreferenceKey,
    revision: u64,
    previous: Option<Entry>,
}

pub struct PreferenceCache {
    salts: [RandomState; 2],
    secret: Option<FingerprintSecret>,
    entries: HashMap<PreferenceKey, Entry>,
    revision: u64,
}

impl Default for PreferenceCache {
    fn default() -> Self {
        Self {
            salts: [RandomState::new(), RandomState::new()],
            secret: FingerprintSecret::generate(),
            entries: HashMap::new(),
            revision: 0,
        }
    }
}

impl PreferenceCache {
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
    /// Language and exact normalized query are part of the namespace. The
    /// caller decides what constitutes a confirmed choice; no text is retained.
    pub fn key(&self, language: &str, query: &str, candidate: &str) -> Option<PreferenceKey> {
        let valid = |text: &str| {
            !text.trim().is_empty()
                && text.len() <= MAX_KEY_BYTES
                && !text.chars().any(char::is_control)
        };
        (valid(language) && language.len() <= 32 && valid(query) && valid(candidate)).then(|| {
            self.secret.as_ref().map_or_else(
                // Entropy failure keeps the old process-local behavior. Such
                // a cache cannot be exported; never use a predictable file key.
                || self.fingerprint(&(language, query, candidate)),
                |secret| secret.fingerprint(&[language, query, candidate]),
            )
        })
    }

    pub(crate) fn fingerprint(&self, value: &impl Hash) -> PreferenceKey {
        PreferenceKey(self.salts.each_ref().map(|salt| salt.hash_one(value)))
    }

    /// A bounded ranking bonus, not a probability. Reads never refresh age or
    /// frequency: displaying/hovering a suggestion cannot reinforce it.
    pub fn bonus(&self, key: PreferenceKey, now: Instant) -> f32 {
        self.entries.get(&key).map_or(0.0, |entry| {
            let frequency = entry.frequency_at(now);
            if frequency < 0.125 {
                0.0
            } else {
                frequency * 3.0
            }
        })
    }

    pub fn record(&mut self, key: PreferenceKey, now: Instant) -> Feedback {
        let previous = self.entries.get(&key).copied();
        if previous.is_none() && self.entries.len() == MAX_ENTRIES {
            // O(capacity) only on a confirmed new choice, never on a lookup.
            let victim = self
                .entries
                .iter()
                .min_by(|(_, a), (_, b)| {
                    a.frequency_at(now)
                        .total_cmp(&b.frequency_at(now))
                        .then(a.revision.cmp(&b.revision))
                })
                .map(|(key, _)| *key)
                .unwrap();
            self.entries.remove(&victim);
        }
        self.revision = self
            .revision
            .checked_add(1)
            .expect("preference revision exhausted");
        self.entries.insert(
            key,
            Entry {
                frequency: (previous.map_or(0.0, |entry| entry.frequency_at(now)) + 1.0)
                    .min(MAX_FREQUENCY),
                updated: now,
                revision: self.revision,
            },
        );
        Feedback {
            owner: self.snapshot_owner(),
            key,
            revision: self.revision,
            previous,
        }
    }

    pub fn undo(&mut self, feedback: Feedback) {
        if feedback.owner != self.snapshot_owner()
            || self
                .entries
                .get(&feedback.key)
                .is_none_or(|entry| entry.revision != feedback.revision)
        {
            return;
        }
        self.revision = self
            .revision
            .checked_add(1)
            .expect("preference revision exhausted");
        if let Some(previous) = feedback.previous {
            self.entries.insert(feedback.key, previous);
        } else {
            self.entries.remove(&feedback.key);
        }
    }

    pub fn clear(&mut self) {
        // A cleared/undone snapshot is newer than every queued older save.
        self.revision = self
            .revision
            .checked_add(1)
            .expect("preference revision exhausted");
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_query_and_instance_isolation_and_bounds() {
        let cache = PreferenceCache::default();
        let key = cache.key("en", "hel", "hello").unwrap();
        assert_eq!(Some(key), cache.key("en", "hel", "hello"));
        assert_ne!(Some(key), cache.key("ja", "hel", "hello"));
        assert_ne!(Some(key), cache.key("en", "he", "hello"));
        assert_ne!(
            Some(key),
            PreferenceCache::default().key("en", "hel", "hello")
        );
        for value in [
            "".to_owned(),
            " ".into(),
            "bad\ntext".into(),
            "a".repeat(MAX_KEY_BYTES + 1),
        ] {
            assert!(cache.key("en", &value, "hello").is_none());
            assert!(cache.key("en", "hel", &value).is_none());
        }
    }

    #[test]
    fn bounded_frequency_decays_without_refresh_on_read() {
        let mut cache = PreferenceCache::default();
        let now = Instant::now();
        let key = cache.key("en", "hel", "hello").unwrap();
        for _ in 0..100 {
            cache.record(key, now);
        }
        assert_eq!(cache.bonus(key, now), 24.0);
        assert_eq!(cache.bonus(key, now + HALF_LIFE), 12.0);
        assert_eq!(cache.bonus(key, now + HALF_LIFE * 2), 6.0);
        assert_eq!(cache.bonus(key, now + HALF_LIFE * 7), 0.0);
    }

    #[test]
    fn eviction_clear_and_stale_undo_are_bounded() {
        let mut cache = PreferenceCache::default();
        let now = Instant::now();
        let key = cache.key("en", "hel", "hello").unwrap();
        let old = cache.record(key, now);
        for n in 0..MAX_ENTRIES {
            cache.record(cache.key("en", "x", &n.to_string()).unwrap(), now);
        }
        assert_eq!(cache.len(), MAX_ENTRIES);
        assert_eq!(cache.bonus(key, now), 0.0);
        cache.record(key, now);
        cache.undo(old);
        assert_eq!(cache.bonus(key, now), 3.0);
        let old = cache.record(key, now);
        cache.clear();
        cache.record(key, now);
        cache.undo(old);
        assert_eq!(cache.bonus(key, now), 3.0);
    }

    #[test]
    fn undo_restores_saturated_and_decayed_frequency_exactly() {
        let mut cache = PreferenceCache::default();
        let now = Instant::now();
        let key = cache.key("en", "hel", "hello").unwrap();
        let first = cache.record(key, now);
        let second = cache.record(key, now + HALF_LIFE);
        assert_eq!(cache.bonus(key, now + HALF_LIFE), 4.5);
        cache.undo(second);
        assert_eq!(cache.bonus(key, now + HALF_LIFE), 1.5);
        cache.undo(first);
        assert!(cache.is_empty());
        for _ in 0..8 {
            cache.record(key, now);
        }
        let saturated = cache.record(key, now);
        cache.undo(saturated);
        assert_eq!(cache.bonus(key, now), 24.0);
    }

    #[test]
    fn entropy_failure_retains_local_learning_but_never_exports_a_predictable_key() {
        let mut cache = PreferenceCache {
            secret: None,
            ..Default::default()
        };
        let now = Instant::now();
        let key = cache.key("en", "hel", "hello").unwrap();
        cache.record(key, now);
        assert_eq!(cache.key("en", "hel", "hello"), Some(key));
        assert_eq!(cache.bonus(key, now), 3.0);
        assert_eq!(
            cache.snapshot_at(now, std::time::SystemTime::now()).err(),
            Some(persistence::PreferenceError::EntropyUnavailable)
        );
    }
}

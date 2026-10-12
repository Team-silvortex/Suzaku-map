//! Versioned, bounded preference snapshots. No filesystem or model dependency.
//! Fingerprints are not anonymization: the file also contains their secret key.
use super::{
    Entry, FingerprintSecret, HALF_LIFE, MAX_ENTRIES, MAX_FREQUENCY, PreferenceCache, PreferenceKey,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt, io,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const MAX_FILE_BYTES: usize = 512 * 1024;
pub const FINGERPRINT_FORMAT: &str = "hmac-sha256-128-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreferenceError {
    EntropyUnavailable,
    InvalidDocument,
    UnsupportedFormat,
    FileTooLarge,
    StagingLimit,
    StartupTimeout,
    ClockUnavailable,
    InvalidPath,
    InsecureFile,
    Busy,
    Conflict,
    WrongCache,
    StaleSnapshot,
    NotEmpty,
    InputBusy,
    PrivateContext,
    Cancelled,
    Io(io::ErrorKind),
}

impl fmt::Display for PreferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EntropyUnavailable => "Cannot obtain a preference fingerprint key",
            Self::InvalidDocument => "Invalid preference document",
            Self::UnsupportedFormat => "Unsupported preference format",
            Self::FileTooLarge => "Preference file exceeds its size limit",
            Self::StagingLimit => "Preference staging exceeds its bounded recovery limit",
            Self::StartupTimeout => "Preference restore timed out; using memory-only learning",
            Self::ClockUnavailable => "Cannot represent the preference timestamp",
            Self::InvalidPath => "Preference storage requires a private absolute file path",
            Self::InsecureFile => "Preference storage requires owned private regular files",
            Self::Busy => "Preference storage is already in use",
            Self::Conflict => "Preference file changed externally; it was not overwritten",
            Self::WrongCache => "Preference snapshot belongs to another cache",
            Self::StaleSnapshot => "An older preference snapshot cannot replace newer state",
            Self::NotEmpty => "Only an empty preference snapshot may clear the file",
            Self::InputBusy => "Preference restore requires an unused empty input context",
            Self::PrivateContext => {
                "Preferences cannot be restored or exported in a private context"
            }
            Self::Cancelled => "Preference operation expired or was superseded before publication",
            Self::Io(_) => "Preference file operation failed; publication may be unconfirmed",
        };
        // Never echo untrusted JSON, fingerprints, input text or filesystem paths.
        formatter.write_str(message)
    }
}

impl std::error::Error for PreferenceError {}

impl From<io::Error> for PreferenceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    fingerprint: String,
    frequency: f32,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format: String,
    schema_version: u32,
    fingerprint_format: String,
    fingerprint_key_hex: String,
    saved_unix_millis: u64,
    entries: Vec<Record>,
}

/// Sensitive opaque export. Pending adoptions, undo history, raw text and the
/// process-only owner/revision guard are never serialized. Serialize off-thread.
#[derive(Clone)]
pub struct PreferenceSnapshot {
    document: Document,
    pub(super) owner: PreferenceKey,
    pub(super) revision: u64,
}

impl PreferenceSnapshot {
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
    pub fn len(&self) -> usize {
        self.document.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.document.entries.is_empty()
    }

    pub fn to_json(&self) -> Result<String, PreferenceError> {
        let raw =
            serde_json::to_string(&self.document).map_err(|_| PreferenceError::InvalidDocument)?;
        if raw.len() > MAX_FILE_BYTES {
            return Err(PreferenceError::FileTooLarge);
        }
        Ok(raw)
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(encoded, "{byte:02x}").unwrap();
    }
    encoded
}

fn unhex<const N: usize>(raw: &str) -> Result<[u8; N], PreferenceError> {
    if raw.len() != N * 2 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PreferenceError::InvalidDocument);
    }
    let mut bytes = [0; N];
    for (index, pair) in raw.as_bytes().chunks_exact(2).enumerate() {
        let digit = |byte: u8| (byte as char).to_digit(16).unwrap() as u8;
        bytes[index] = digit(pair[0]) * 16 + digit(pair[1]);
    }
    Ok(bytes)
}

fn key_bytes(key: PreferenceKey) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&key.0[0].to_be_bytes());
    bytes[8..].copy_from_slice(&key.0[1].to_be_bytes());
    bytes
}

fn key_from_hex(raw: &str) -> Result<PreferenceKey, PreferenceError> {
    let bytes = unhex::<16>(raw)?;
    Ok(PreferenceKey([
        u64::from_be_bytes(bytes[..8].try_into().unwrap()),
        u64::from_be_bytes(bytes[8..].try_into().unwrap()),
    ]))
}

impl PreferenceCache {
    pub(super) fn snapshot_owner(&self) -> PreferenceKey {
        // Unlike the stable HMAC key, this owner changes on every load/process.
        self.fingerprint(&"suzaku-preference-snapshot-owner-v1")
    }

    /// Capture at most MAX_ENTRIES already-decayed rows, without doing any I/O.
    pub fn snapshot_at(
        &self,
        now: Instant,
        wall: SystemTime,
    ) -> Result<PreferenceSnapshot, PreferenceError> {
        let secret = self
            .secret
            .as_ref()
            .ok_or(PreferenceError::EntropyUnavailable)?;
        let saved_unix_millis = wall
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PreferenceError::ClockUnavailable)?
            .as_millis()
            .try_into()
            .map_err(|_| PreferenceError::ClockUnavailable)?;
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(key, entry)| {
                let frequency = entry.frequency_at(now);
                (frequency >= 0.125).then(|| Record {
                    fingerprint: hex(&key_bytes(*key)),
                    frequency,
                })
            })
            .collect();
        entries.sort_by(|left, right| left.fingerprint.cmp(&right.fingerprint));
        Ok(PreferenceSnapshot {
            document: Document {
                format: "suzaku-preferences".into(),
                schema_version: 1,
                fingerprint_format: FINGERPRINT_FORMAT.into(),
                fingerprint_key_hex: hex(&secret.bytes),
                saved_unix_millis,
                entries,
            },
            owner: self.snapshot_owner(),
            revision: self.revision,
        })
    }

    /// Strict, all-or-nothing restore. Wall time accounts for time while stopped;
    /// monotonic time resumes in-process decay. A backward clock cannot grow counts.
    pub fn from_json_at(
        raw: &str,
        now: Instant,
        wall: SystemTime,
    ) -> Result<Self, PreferenceError> {
        if raw.len() > MAX_FILE_BYTES {
            return Err(PreferenceError::FileTooLarge);
        }
        let document: Document =
            serde_json::from_str(raw).map_err(|_| PreferenceError::InvalidDocument)?;
        if document.format != "suzaku-preferences"
            || document.schema_version != 1
            || document.fingerprint_format != FINGERPRINT_FORMAT
        {
            return Err(PreferenceError::UnsupportedFormat);
        }
        if document.entries.len() > MAX_ENTRIES {
            return Err(PreferenceError::InvalidDocument);
        }
        let secret = FingerprintSecret::from_bytes(unhex::<32>(&document.fingerprint_key_hex)?);
        let saved = UNIX_EPOCH
            .checked_add(Duration::from_millis(document.saved_unix_millis))
            .ok_or(PreferenceError::InvalidDocument)?;
        let elapsed = wall.duration_since(saved).unwrap_or_default();
        let decay = (-elapsed.as_secs_f32() / HALF_LIFE.as_secs_f32()).exp2();
        let mut seen = std::collections::HashSet::new();
        let mut entries = HashMap::new();
        for record in document.entries {
            let key = key_from_hex(&record.fingerprint)?;
            if !record.frequency.is_finite()
                || record.frequency <= 0.0
                || record.frequency > MAX_FREQUENCY
                || !seen.insert(key)
            {
                return Err(PreferenceError::InvalidDocument);
            }
            let frequency = record.frequency * decay;
            if frequency >= 0.125 {
                entries.insert(
                    key,
                    Entry {
                        frequency,
                        updated: now,
                        revision: entries.len() as u64 + 1,
                    },
                );
            }
        }
        let revision = entries.len() as u64;
        // Keep independent process-local prefix/owner salts, without requiring
        // fresh entropy for the already validated, persisted HMAC key.
        Ok(Self {
            salts: Default::default(),
            secret: Some(secret),
            entries,
            revision,
        })
    }
}

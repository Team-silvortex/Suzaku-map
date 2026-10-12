//! Native-host policy only; file work is owned by the model-independent writer.
use crate::preferences::{
    persistence::PreferenceError,
    store::FILE_NAME,
    worker::{PreferenceWriter, WriterStatus},
};
use std::{ffi::OsStr, path::PathBuf, time::Duration};

#[derive(Default)]
pub(super) struct HostPreferences {
    pub writer: Option<PreferenceWriter>,
    pub enabled: bool,
    pub error: Option<PreferenceError>,
    pub queued_revision: Option<u64>,
}

impl HostPreferences {
    pub fn json(&self, entries: usize) -> serde_json::Value {
        let status = self
            .writer
            .as_ref()
            .map(PreferenceWriter::status)
            .unwrap_or(WriterStatus {
                epoch: String::new(),
                memory_only: true,
                pending: false,
                error: self.error,
                clear: None,
            });
        serde_json::json!({
            "enabled": self.enabled,
            "epoch": status.epoch,
            "memory_only": status.memory_only,
            "pending": status.pending,
            "error": status.error.or(self.error).map(|error| error.to_string()),
            "entries": entries,
            "clear": status.clear.map(|clear| serde_json::json!({
                "id": clear.id, "state": clear.state,
                "error": clear.error.map(|error| error.to_string())
            }))
        })
    }
}

fn mode(value: Option<&OsStr>, legacy_memory_fixture: bool) -> Result<bool, PreferenceError> {
    match value {
        Some(value) if value == "memory" => Ok(false),
        Some(value) if value == "disk" => Ok(true),
        Some(_) => Err(PreferenceError::InvalidPath),
        // The original native regression fixture explicitly tests RAM-only
        // learning. Its existing QA marker selects that policy, not a production
        // exception to disk validation. Dedicated disk QA uses ordinary defaults.
        None => Ok(!legacy_memory_fixture),
    }
}

pub(super) fn current_mode() -> Result<bool, PreferenceError> {
    mode(
        std::env::var_os("SUZAKU_PREFERENCE_MODE").as_deref(),
        std::env::var_os("SUZAKU_NATIVE_SYNC_QA").is_some_and(|value| value == "1"),
    )
}

pub(super) fn current_path() -> Result<PathBuf, PreferenceError> {
    if std::env::var_os("SUZAKU_IME_CONFIG").is_some() {
        // An explicit isolated configuration must never fall back to HOME/state.
        let settings = crate::ime::settings::settings_path()
            .filter(|path| path.is_absolute())
            .ok_or(PreferenceError::InvalidPath)?;
        return Ok(settings
            .parent()
            .ok_or(PreferenceError::InvalidPath)?
            .join("candidate-preferences")
            .join(FILE_NAME));
    }
    Ok(
        crate::data::paths::xdg_home("XDG_STATE_HOME", ".local/state")
            .ok_or(PreferenceError::InvalidPath)?
            .join("suzaku-ime")
            .join(FILE_NAME),
    )
}

/// Bootstrap finishes before any input engine attaches. No late restore may
/// overwrite a draft; the file worker itself performs all parsing/disk work.
pub fn initialize() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    if INITIALIZED.swap(true, Ordering::AcqRel) {
        return;
    }
    let mode = current_mode();
    if mode == Ok(false) {
        return;
    }
    let opened = mode
        .and_then(|_| current_path())
        .and_then(|path| PreferenceWriter::start(path, Duration::from_millis(500)));
    super::with_shared_host_ime_session(|session| {
        session.preferences.enabled = true;
        match opened {
            Ok((writer, cache)) => match session.engine.install_preference_cache(cache) {
                Ok(()) => {
                    session.preferences.queued_revision =
                        Some(session.engine.preference_revision());
                    session.preferences.writer = Some(writer);
                }
                Err(error) => session.preferences.error = Some(error),
            },
            Err(error) => session.preferences.error = Some(error),
        }
    });
}

/// Only after the IBus main loop has stopped, never from a keyboard callback.
pub fn shutdown() {
    let writer = super::with_shared_host_ime_session(|session| session.preferences.writer.take());
    if let Some(writer) = writer
        && !writer.shutdown(Duration::from_millis(700))
    {
        eprintln!("Suzaku: preference shutdown was not confirmed; latest changes may be unsaved");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_defaults_to_disk_and_legacy_ram_qa_is_explicitly_overridable() {
        assert_eq!(mode(None, false), Ok(true));
        assert_eq!(mode(None, true), Ok(false));
        assert_eq!(mode(Some(OsStr::new("disk")), true), Ok(true));
        assert_eq!(mode(Some(OsStr::new("memory")), false), Ok(false));
        for value in ["", "Disk", "unknown"] {
            assert!(mode(Some(OsStr::new(value)), false).is_err());
        }
    }
}

//! Synthetic owned files and cache snapshots only; never the user's cache.
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};
use suzaku_map::preferences::{
    HALF_LIFE, MAX_ENTRIES, PreferenceCache,
    persistence::{MAX_FILE_BYTES, PreferenceError},
};

fn wall() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(2_000_000_000)
}

fn learned(now: Instant) -> PreferenceCache {
    let mut cache = PreferenceCache::default();
    let key = cache.key("zh-Hans", "shijian", "实践").unwrap();
    for _ in 0..8 {
        cache.record(key, now);
    }
    cache
}

#[test]
fn fingerprints_survive_restore_and_keep_language_query_and_tuple_boundaries() {
    let now = Instant::now();
    let cache = learned(now);
    let key = cache.key("zh-Hans", "shijian", "实践").unwrap();
    let raw = cache.snapshot_at(now, wall()).unwrap().to_json().unwrap();
    assert!(!raw.contains("shijian"));
    assert!(!raw.contains("实践"));
    assert!(!raw.contains("zh-Hans"));
    assert!(!raw.contains("pending"));
    assert!(!raw.contains("undo"));
    let restored = PreferenceCache::from_json_at(&raw, now, wall()).unwrap();
    assert_eq!(restored.key("zh-Hans", "shijian", "实践"), Some(key));
    assert_eq!(restored.bonus(key, now), 24.0);
    assert_ne!(restored.key("en", "shijian", "实践"), Some(key));
    assert_ne!(restored.key("zh-Hans", "shi jian", "实践"), Some(key));
    assert_ne!(restored.key("en", "ab", "c"), restored.key("en", "a", "bc"));
    assert_ne!(
        PreferenceCache::default().key("zh-Hans", "shijian", "实践"),
        Some(key)
    );
}

#[test]
fn stopped_time_and_monotonic_time_both_decay_without_read_refresh() {
    let now = Instant::now();
    let cache = learned(now);
    let key = cache.key("zh-Hans", "shijian", "实践").unwrap();
    let snapshot = cache.snapshot_at(now + HALF_LIFE, wall()).unwrap();
    let raw = snapshot.to_json().unwrap();
    let loaded_at = now + HALF_LIFE * 3;
    let restored = PreferenceCache::from_json_at(&raw, loaded_at, wall() + HALF_LIFE).unwrap();
    assert_eq!(restored.bonus(key, loaded_at), 6.0);
    assert_eq!(restored.bonus(key, loaded_at + HALF_LIFE), 3.0);
    assert_eq!(restored.bonus(key, loaded_at + HALF_LIFE), 3.0);
    let expired = PreferenceCache::from_json_at(&raw, loaded_at, wall() + HALF_LIFE * 7).unwrap();
    assert!(expired.is_empty());
    let backward = PreferenceCache::from_json_at(&raw, loaded_at, wall() - HALF_LIFE).unwrap();
    assert_eq!(backward.bonus(key, loaded_at), 12.0);
    assert!(
        cache
            .snapshot_at(now, UNIX_EPOCH - Duration::from_secs(1))
            .is_err()
    );
}

#[test]
fn unknown_malformed_duplicate_and_over_budget_documents_fail_whole_restore() {
    let now = Instant::now();
    let raw = learned(now)
        .snapshot_at(now, wall())
        .unwrap()
        .to_json()
        .unwrap();
    let original: serde_json::Value = serde_json::from_str(&raw).unwrap();
    // Keep malformed values independent of serialization's own validation.
    for (field, replacement) in [
        ("format", serde_json::json!("other")),
        ("schema_version", serde_json::json!(2)),
        ("fingerprint_format", serde_json::json!("std-hasher")),
        ("fingerprint_key_hex", serde_json::json!("not hex")),
        ("saved_unix_millis", serde_json::json!(-1)),
        ("extra", serde_json::json!("synthetic private text")),
    ] {
        let mut value = original.clone();
        value[field] = replacement;
        let error = PreferenceCache::from_json_at(&value.to_string(), now, wall())
            .err()
            .unwrap();
        assert!(!error.to_string().contains("synthetic private text"));
    }
    for frequency in [
        serde_json::json!(0),
        serde_json::json!(-1),
        serde_json::json!(8.1),
        serde_json::Value::Null,
        serde_json::json!("text"),
    ] {
        let mut value = original.clone();
        value["entries"][0]["frequency"] = frequency;
        assert!(PreferenceCache::from_json_at(&value.to_string(), now, wall()).is_err());
    }
    let mut duplicate = original.clone();
    let entry = duplicate["entries"][0].clone();
    duplicate["entries"]
        .as_array_mut()
        .unwrap()
        .push(entry.clone());
    assert!(PreferenceCache::from_json_at(&duplicate.to_string(), now, wall()).is_err());
    let mut excessive = original.clone();
    excessive["entries"] = serde_json::Value::Array(vec![entry; MAX_ENTRIES + 1]);
    assert!(PreferenceCache::from_json_at(&excessive.to_string(), now, wall()).is_err());
    for broken in [
        "{".to_owned(),
        format!("{} ", "x".repeat(MAX_FILE_BYTES + 1)),
        raw.replace(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
        ),
    ] {
        assert!(PreferenceCache::from_json_at(&broken, now, wall()).is_err());
    }
}

#[test]
fn capacity_survives_round_trip_and_undo_after_restore_only_affects_new_feedback() {
    let now = Instant::now();
    let mut cache = PreferenceCache::default();
    for index in 0..MAX_ENTRIES + 5 {
        cache.record(
            cache.key("en", "x", &format!("choice {index}")).unwrap(),
            now,
        );
    }
    let raw = cache.snapshot_at(now, wall()).unwrap().to_json().unwrap();
    assert!(raw.len() < MAX_FILE_BYTES);
    let mut restored = PreferenceCache::from_json_at(&raw, now, wall()).unwrap();
    assert_eq!(restored.len(), MAX_ENTRIES);
    let key = restored
        .key("en", "x", &format!("choice {}", MAX_ENTRIES + 4))
        .unwrap();
    assert_eq!(restored.bonus(key, now), 3.0);
    let receipt = restored.record(key, now);
    assert_eq!(restored.bonus(key, now), 6.0);
    restored.undo(receipt);
    assert_eq!(restored.bonus(key, now), 3.0);
}

#[test]
fn restored_cache_ignores_foreign_feedback_receipts_with_matching_keys_and_revisions() {
    let now = Instant::now();
    let mut original = PreferenceCache::default();
    let key = original.key("en", "hel", "hello").unwrap();
    let old_receipt = original.record(key, now);
    let raw = original
        .snapshot_at(now, wall())
        .unwrap()
        .to_json()
        .unwrap();
    let mut restored = PreferenceCache::from_json_at(&raw, now, wall()).unwrap();
    assert_eq!(restored.key("en", "hel", "hello"), Some(key));
    assert_eq!(restored.bonus(key, now), 3.0);
    restored.undo(old_receipt);
    assert_eq!(
        restored.len(),
        1,
        "an old cache's receipt must not remove a restored entry"
    );
    assert_eq!(restored.bonus(key, now), 3.0);
    let current = restored.record(key, now);
    assert_eq!(restored.bonus(key, now), 6.0);
    restored.undo(current);
    assert_eq!(restored.bonus(key, now), 3.0);
}

fn engine(language: &str) -> XRTabletImeEngine {
    XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    })
}

#[test]
fn cold_engine_restore_remembers_choices_without_restoring_drafts_or_history() {
    let mut first = engine("zh-Hans");
    first.enable_ibus_candidate_mix();
    for _ in 0..8 {
        first.seed("shijian");
        let index = first
            .candidates()
            .iter()
            .position(|row| row.text == "实践")
            .unwrap();
        first.select_candidate(index);
        assert!(first.commit(CommitOptions { force: true }).ok);
        first.clear_session_context();
    }
    let raw = first.preference_snapshot().unwrap().to_json().unwrap();
    let restored = PreferenceCache::from_json_at(&raw, Instant::now(), SystemTime::now()).unwrap();
    let mut second = engine("zh-Hans");
    second.install_preference_cache(restored).unwrap();
    assert!(second.snapshot().seed_text.is_empty());
    assert!(second.snapshot().committed_text.is_empty());
    assert!(second.undo().is_none());
    second.enable_ibus_candidate_mix();
    second.seed("shijian");
    assert_eq!(second.candidates()[0].text, "实践");
    second.seed("shi jian");
    assert_eq!(second.candidates()[0].text, "时间");
}

#[test]
fn late_restore_and_private_exports_do_not_disturb_live_input() {
    let now = Instant::now();
    let raw = learned(now)
        .snapshot_at(now, wall())
        .unwrap()
        .to_json()
        .unwrap();
    let mut active = engine("en");
    active.enable_ibus_candidate_mix();
    active.seed("hel");
    active.select_candidate(1);
    let before = active.snapshot();
    let pool = active.candidates().to_vec();
    let replacement = PreferenceCache::from_json_at(&raw, now, wall()).unwrap();
    assert_eq!(
        active.install_preference_cache(replacement),
        Err(PreferenceError::InputBusy)
    );
    assert_eq!(active.snapshot(), before);
    assert_eq!(active.candidates(), pool);
    assert!(active.selected_completion_text(true).is_some());
    active.set_preferences_enabled(false);
    assert_eq!(
        active.preference_snapshot().err(),
        Some(PreferenceError::PrivateContext)
    );
    let mut private = engine("en");
    private.set_preferences_enabled(false);
    assert_eq!(
        private.install_preference_cache(PreferenceCache::default()),
        Err(PreferenceError::PrivateContext)
    );
}

#[test]
fn unconfirmed_and_undone_choices_are_absent_from_exports() {
    let mut engine = engine("zh-Hans");
    engine.enable_ibus_candidate_mix();
    engine.seed("shijian");
    let index = engine
        .candidates()
        .iter()
        .position(|row| row.text == "实践")
        .unwrap();
    engine.select_candidate(index);
    engine.stage_selected_preference();
    engine.seed("实践");
    assert!(engine.preference_snapshot().unwrap().is_empty());
    engine.select_candidate(0);
    assert!(engine.commit(CommitOptions { force: true }).ok);
    assert_eq!(engine.preference_snapshot().unwrap().len(), 1);
    engine.undo().unwrap();
    assert!(engine.preference_snapshot().unwrap().is_empty());
}

#[cfg(target_os = "linux")]
mod files {
    use super::*;
    use std::{
        fs,
        io::Write,
        os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    use suzaku_map::preferences::store::{FILE_NAME, PreferenceStore};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Owned(PathBuf);
    impl Owned {
        fn new() -> Self {
            for _ in 0..100 {
                let path = std::env::temp_dir().join(format!(
                    "szpref-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::DirBuilder::new().mode(0o700).create(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("cannot reserve owned preference fixture: {error}"),
                }
            }
            panic!("cannot reserve owned preference fixture");
        }
        fn path(&self) -> PathBuf {
            self.0.join(FILE_NAME)
        }
    }
    impl Drop for Owned {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn private_write(path: &std::path::Path, text: &str) {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    #[test]
    fn actual_file_roundtrip_is_private_locked_and_model_independent() {
        let owned = Owned::new();
        let now = Instant::now();
        let (mut store, mut cache) = PreferenceStore::open_at(owned.path(), now, wall()).unwrap();
        let key = cache.key("en", "hel", "help").unwrap();
        cache.record(key, now);
        store
            .save(&cache.snapshot_at(now, wall()).unwrap())
            .unwrap();
        store
            .save(&cache.snapshot_at(now, wall()).unwrap())
            .unwrap();
        assert_eq!(
            fs::metadata(owned.path()).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            PreferenceStore::open(owned.path()).err(),
            Some(PreferenceError::Busy)
        );
        assert!(
            store
                .save(&PreferenceCache::default().snapshot_at(now, wall()).unwrap())
                .is_err()
        );
        drop(store);
        let (_next, restored) = PreferenceStore::open_at(owned.path(), now, wall()).unwrap();
        assert_eq!(restored.key("en", "hel", "help"), Some(key));
        assert_eq!(restored.bonus(key, now), 3.0);
        assert!(!fs::read_to_string(owned.path()).unwrap().contains("help"));
    }

    #[test]
    fn clear_and_undo_reject_older_queued_snapshots_and_leave_no_temporary_files() {
        let owned = Owned::new();
        let now = Instant::now();
        let (mut store, mut cache) = PreferenceStore::open_at(owned.path(), now, wall()).unwrap();
        let key = cache.key("en", "hel", "hello").unwrap();
        let receipt = cache.record(key, now);
        let old = cache.snapshot_at(now, wall()).unwrap();
        store.save(&old).unwrap();
        assert_eq!(store.clear(&old), Err(PreferenceError::NotEmpty));
        cache.undo(receipt);
        store
            .save(&cache.snapshot_at(now, wall()).unwrap())
            .unwrap();
        assert_eq!(store.save(&old), Err(PreferenceError::StaleSnapshot));
        cache.record(key, now);
        let before_clear = cache.snapshot_at(now, wall()).unwrap();
        store.save(&before_clear).unwrap();
        cache.clear();
        let empty = cache.snapshot_at(now, wall()).unwrap();
        store.clear(&empty).unwrap();
        assert!(!owned.path().exists());
        assert_eq!(
            store.save(&before_clear),
            Err(PreferenceError::StaleSnapshot)
        );
        store.clear(&empty).unwrap();
        assert!(fs::read_dir(&owned.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        drop(store);
        let (_next, restored) = PreferenceStore::open_at(owned.path(), now, wall()).unwrap();
        assert!(restored.is_empty());
    }

    #[test]
    fn an_expiry_empty_clear_also_blocks_same_revision_nonempty_siblings() {
        let owned = Owned::new();
        let now = Instant::now();
        let (mut store, mut cache) = PreferenceStore::open_at(owned.path(), now, wall()).unwrap();
        let key = cache.key("en", "hel", "hello").unwrap();
        cache.record(key, now);
        let old = cache.snapshot_at(now, wall()).unwrap();
        store.save(&old).unwrap();
        let expired = cache
            .snapshot_at(now + HALF_LIFE * 7, wall() + HALF_LIFE * 7)
            .unwrap();
        assert!(expired.is_empty());
        store.clear(&expired).unwrap();
        assert_eq!(store.save(&old), Err(PreferenceError::StaleSnapshot));
        assert_eq!(store.save(&expired), Err(PreferenceError::StaleSnapshot));
        assert!(!owned.path().exists());
        cache.record(key, now + HALF_LIFE * 7);
        store
            .save(
                &cache
                    .snapshot_at(now + HALF_LIFE * 7, wall() + HALF_LIFE * 7)
                    .unwrap(),
            )
            .unwrap();
        assert!(owned.path().exists());
    }

    #[test]
    fn malformed_files_and_external_edits_are_preserved_instead_of_replaced() {
        let owned = Owned::new();
        private_write(&owned.path(), "{broken synthetic document");
        assert_eq!(
            PreferenceStore::open(owned.path()).err(),
            Some(PreferenceError::InvalidDocument)
        );
        assert_eq!(
            fs::read_to_string(owned.path()).unwrap(),
            "{broken synthetic document"
        );
        fs::remove_file(owned.path()).unwrap();
        let (mut store, mut cache) = PreferenceStore::open(owned.path()).unwrap();
        cache.record(cache.key("en", "hel", "hello").unwrap(), Instant::now());
        let snapshot = cache
            .snapshot_at(Instant::now(), SystemTime::now())
            .unwrap();
        store.save(&snapshot).unwrap();
        fs::write(owned.path(), "external synthetic edit").unwrap();
        assert_eq!(store.save(&snapshot), Err(PreferenceError::Conflict));
        cache.clear();
        assert_eq!(
            store.clear(
                &cache
                    .snapshot_at(Instant::now(), SystemTime::now())
                    .unwrap()
            ),
            Err(PreferenceError::Conflict)
        );
        assert_eq!(
            fs::read_to_string(owned.path()).unwrap(),
            "external synthetic edit"
        );
    }

    #[test]
    fn restart_recovers_only_private_preference_staging_under_its_exclusive_lease() {
        let owned = Owned::new();
        let (mut store, mut cache) = PreferenceStore::open(owned.path()).unwrap();
        cache.record(cache.key("en", "hel", "help").unwrap(), Instant::now());
        store
            .save(
                &cache
                    .snapshot_at(Instant::now(), SystemTime::now())
                    .unwrap(),
            )
            .unwrap();
        let original = fs::read(owned.path()).unwrap();
        let orphan = owned.0.join(".suzaku-preferences-123-0.tmp");
        private_write(&orphan, "partial synthetic preference data");
        let sentinels = [
            ".suzaku-123-0.tmp",
            ".suzaku-settings-123-0.tmp",
            ".suzaku-preferences-0123-0.tmp",
            ".suzaku-preferences-0-0.tmp",
            ".suzaku-preferences-123-00.tmp",
            ".suzaku-preferences-123-x.tmp",
            ".suzaku-preferences-4294967296-0.tmp",
            "unrelated.json",
        ];
        for name in sentinels {
            private_write(&owned.0.join(name), "synthetic keep");
        }
        assert_eq!(
            PreferenceStore::open(owned.path()).err(),
            Some(PreferenceError::Busy)
        );
        assert!(orphan.exists());
        drop(store);
        let (_next, restored) = PreferenceStore::open(owned.path()).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(fs::read(owned.path()).unwrap(), original);
        assert!(!orphan.exists());
        for name in sentinels {
            assert_eq!(
                fs::read_to_string(owned.0.join(name)).unwrap(),
                "synthetic keep"
            );
        }
    }

    #[test]
    fn explicit_clear_and_corrupt_reset_clean_their_staging_only_after_validation() {
        let owned = Owned::new();
        let (mut store, mut cache) = PreferenceStore::open(owned.path()).unwrap();
        cache.record(cache.key("en", "hel", "help").unwrap(), Instant::now());
        store
            .save(
                &cache
                    .snapshot_at(Instant::now(), SystemTime::now())
                    .unwrap(),
            )
            .unwrap();
        let original = fs::read(owned.path()).unwrap();
        let orphan = owned.0.join(".suzaku-preferences-123-1.tmp");
        private_write(&orphan, "partial synthetic data");
        let keep = owned.0.join(".suzaku-123-1.tmp");
        private_write(&keep, "synthetic keep");
        cache.clear();
        let empty = cache
            .snapshot_at(Instant::now(), SystemTime::now())
            .unwrap();
        assert_eq!(
            store.clear_if(&empty, || false),
            Err(PreferenceError::Cancelled)
        );
        assert!(orphan.exists());
        assert_eq!(fs::read(owned.path()).unwrap(), original);
        store.clear(&empty).unwrap();
        assert!(!owned.path().exists() && !orphan.exists());
        drop(store);
        private_write(&owned.path(), "synthetic corrupt document");
        private_write(&orphan, "another synthetic partial document");
        assert_eq!(
            PreferenceStore::reset_if(owned.path(), &empty, || false).err(),
            Some(PreferenceError::Cancelled)
        );
        assert!(orphan.exists());
        assert_eq!(
            fs::read_to_string(owned.path()).unwrap(),
            "synthetic corrupt document"
        );
        let _reset = PreferenceStore::reset_if(owned.path(), &empty, || true).unwrap();
        assert!(!owned.path().exists() && !orphan.exists());
        assert_eq!(fs::read_to_string(keep).unwrap(), "synthetic keep");
    }

    #[test]
    fn unsafe_staging_rejects_the_entire_cleanup_without_deleting_any_file() {
        for kind in ["symlink", "hardlink", "fifo", "public", "directory"] {
            let owned = Owned::new();
            let (mut store, cache) = PreferenceStore::open(owned.path()).unwrap();
            let empty = cache
                .snapshot_at(Instant::now(), SystemTime::now())
                .unwrap();
            store.save(&empty).unwrap();
            let original = fs::read(owned.path()).unwrap();
            drop(store);
            let good = owned.0.join(".suzaku-preferences-123-1.tmp");
            let bad = owned.0.join(".suzaku-preferences-123-2.tmp");
            let keep = owned.0.join("synthetic-sentinel");
            private_write(&good, "synthetic valid staging");
            private_write(&keep, "synthetic keep");
            match kind {
                "symlink" => std::os::unix::fs::symlink(&keep, &bad).unwrap(),
                "hardlink" => fs::hard_link(&keep, &bad).unwrap(),
                "fifo" => {
                    let name = std::ffi::CString::new(bad.as_os_str().as_encoded_bytes()).unwrap();
                    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
                }
                "public" => {
                    private_write(&bad, "synthetic public staging");
                    fs::set_permissions(&bad, fs::Permissions::from_mode(0o644)).unwrap();
                }
                "directory" => fs::create_dir(&bad).unwrap(),
                _ => unreachable!(),
            }
            assert_eq!(
                PreferenceStore::open(owned.path()).err(),
                Some(PreferenceError::InsecureFile)
            );
            assert_eq!(
                PreferenceStore::reset_if(owned.path(), &empty, || true).err(),
                Some(PreferenceError::InsecureFile)
            );
            assert_eq!(fs::read(owned.path()).unwrap(), original);
            assert!(good.exists() && fs::symlink_metadata(bad).is_ok());
            assert_eq!(fs::read_to_string(keep).unwrap(), "synthetic keep");
        }
    }

    #[test]
    fn staging_and_directory_budgets_fail_before_partial_cleanup() {
        for (count, staging) in [(65, true), (129, false)] {
            let owned = Owned::new();
            for sequence in 0..count {
                let name = if staging {
                    format!(".suzaku-preferences-123-{sequence}.tmp")
                } else {
                    format!("keep-{sequence}")
                };
                private_write(&owned.0.join(name), "synthetic keep");
            }
            let empty = PreferenceCache::default()
                .snapshot_at(Instant::now(), SystemTime::now())
                .unwrap();
            assert_eq!(
                PreferenceStore::open(owned.path()).err(),
                Some(PreferenceError::StagingLimit)
            );
            assert_eq!(
                PreferenceStore::reset_if(owned.path(), &empty, || true).err(),
                Some(PreferenceError::StagingLimit)
            );
            assert_eq!(fs::read_dir(&owned.0).unwrap().count(), count + 1); // only the lease was created
            assert!(!owned.path().exists());
        }
    }

    #[test]
    fn symlinks_hardlinks_fifos_and_public_permissions_are_rejected() {
        let owned = Owned::new();
        let outside = owned.0.join("keep.json");
        private_write(&outside, "synthetic outside sentinel");
        std::os::unix::fs::symlink(&outside, owned.path()).unwrap();
        assert!(PreferenceStore::open(owned.path()).is_err());
        fs::remove_file(owned.path()).unwrap();
        fs::hard_link(&outside, owned.path()).unwrap();
        assert_eq!(
            PreferenceStore::open(owned.path()).err(),
            Some(PreferenceError::InsecureFile)
        );
        fs::remove_file(owned.path()).unwrap();
        let fifo = std::ffi::CString::new(owned.path().as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let began = Instant::now();
        assert_eq!(
            PreferenceStore::open(owned.path()).err(),
            Some(PreferenceError::InsecureFile)
        );
        assert!(began.elapsed() < Duration::from_secs(1));
        fs::remove_file(owned.path()).unwrap();
        private_write(&owned.path(), "{}");
        fs::set_permissions(owned.path(), fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            PreferenceStore::open(owned.path()).err(),
            Some(PreferenceError::InsecureFile)
        );
        assert_eq!(
            fs::read_to_string(&outside).unwrap(),
            "synthetic outside sentinel"
        );
    }

    #[test]
    fn unsafe_parents_and_relative_reserved_paths_do_not_create_cache_files() {
        let owned = Owned::new();
        assert_eq!(
            PreferenceStore::open("relative.json").err(),
            Some(PreferenceError::InvalidPath)
        );
        assert_eq!(
            PreferenceStore::open(owned.0.join(".suzaku-data.lock")).err(),
            Some(PreferenceError::InvalidPath)
        );
        let public = owned.0.join("public");
        fs::create_dir(&public).unwrap();
        fs::set_permissions(&public, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            PreferenceStore::open(public.join(FILE_NAME)).err(),
            Some(PreferenceError::InsecureFile)
        );
        assert!(!public.join(FILE_NAME).exists());
        let alias = owned.0.join("alias");
        std::os::unix::fs::symlink(&public, &alias).unwrap();
        assert_eq!(
            PreferenceStore::open(alias.join(FILE_NAME)).err(),
            Some(PreferenceError::InsecureFile)
        );
    }

    #[test]
    fn two_real_processes_reuse_fingerprints_without_process_salts() {
        const ROOT: &str = "SUZAKU_PREFERENCE_QA_OWNED_ROOT";
        const PHASE: &str = "SUZAKU_PREFERENCE_QA_CHILD_PHASE";
        if let Ok(root) = std::env::var(ROOT) {
            // An ambient variable alone cannot authorize fixture writes. Only
            // our same-test-executable parent may launch this owned child.
            let root = PathBuf::from(root);
            let parent = unsafe { libc::getppid() };
            assert_eq!(
                fs::read_link(format!("/proc/{parent}/exe")).unwrap(),
                std::env::current_exe().unwrap()
            );
            assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
            assert!(
                root.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(&format!("szpref-{parent}-"))
            );
            let path = root.join(FILE_NAME);
            let now = Instant::now();
            let (mut store, mut cache) = PreferenceStore::open_at(&path, now, wall()).unwrap();
            let key = cache.key("zh-Hans", "shijian", "实践").unwrap();
            match std::env::var(PHASE).unwrap().as_str() {
                "write" => {
                    cache.record(key, now);
                    store
                        .save(&cache.snapshot_at(now, wall()).unwrap())
                        .unwrap();
                }
                "read" => {
                    assert_eq!(cache.len(), 1);
                    assert_eq!(cache.bonus(key, now), 3.0);
                }
                _ => panic!("unknown owned child phase"),
            }
            return;
        }
        let owned = Owned::new();
        for phase in ["write", "read"] {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "files::two_real_processes_reuse_fingerprints_without_process_salts",
                    "--nocapture",
                ])
                .env(ROOT, &owned.0)
                .env(PHASE, phase)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "owned child {phase} failed: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}

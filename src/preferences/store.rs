//! Linux private, single-writer preference files. These methods perform blocking
//! I/O: hosts must call them on their bounded storage worker, not an input thread.
use super::{
    PreferenceCache, PreferenceKey,
    persistence::{MAX_FILE_BYTES, PreferenceError, PreferenceSnapshot},
};
use crate::data::files::{StagedFile, private_directory};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{ErrorKind, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    time::{Instant, SystemTime},
};

pub const FILE_NAME: &str = "preferences-v1.json";
const LOCK_NAME: &str = ".suzaku-preferences.lock";
const STAGING_TAG: &str = "preferences";
const STAGING_PREFIX: &str = ".suzaku-preferences-";
const MAX_STAGING_FILES: usize = 64;
const MAX_DIRECTORY_ENTRIES: usize = 128;

/// One cache owner for this lease. No merging or last-writer-wins across hosts.
pub struct PreferenceStore {
    path: PathBuf,
    baseline: Option<String>,
    owner: PreferenceKey,
    accepted_revision: u64,
    cleared_revision: Option<u64>,
    _lease: PreferenceLease,
}

/// `flock` is attached to an open file description, which a forked child can
/// inherit even when the descriptor is close-on-exec. Explicitly unlock when
/// the owner drops the lease so a child waiting to exec cannot extend it.
/// Only the process which acquired the lock may unlock it: unlocking from a
/// forked child would also release its parent's still-active lock.
struct PreferenceLease {
    file: File,
    owner_pid: libc::pid_t,
}

fn lock_error(error: TryLockError) -> PreferenceError {
    match error {
        TryLockError::WouldBlock => PreferenceError::Busy,
        TryLockError::Error(error) => error.into(),
    }
}

impl Drop for PreferenceLease {
    fn drop(&mut self) {
        if unsafe { libc::getpid() } != self.owner_pid {
            return;
        }
        // Drop cannot report I/O errors. Retry interruptions; on any other
        // error closing the file still provides the normal OS cleanup fallback.
        loop {
            match self.file.unlock() {
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                _ => break,
            }
        }
    }
}

fn private_metadata(metadata: &fs::Metadata) -> Result<(), PreferenceError> {
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(PreferenceError::InsecureFile);
    }
    Ok(())
}

fn read(path: &Path) -> Result<Option<String>, PreferenceError> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    private_metadata(&file.metadata()?)?;
    let mut raw = String::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_string(&mut raw)?;
    if raw.len() > MAX_FILE_BYTES {
        return Err(PreferenceError::FileTooLarge);
    }
    Ok(Some(raw))
}

fn prepare_parent(path: &Path) -> Result<(), PreferenceError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || path
            .file_name()
            .is_none_or(|name| name.to_string_lossy().starts_with(".suzaku-"))
    {
        return Err(PreferenceError::InvalidPath);
    }
    let parent = path.parent().ok_or(PreferenceError::InvalidPath)?;
    let check_ancestors = || {
        for ancestor in parent.ancestors() {
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
                    return Err(PreferenceError::InsecureFile);
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    };
    check_ancestors()?;
    private_directory(parent)?;
    check_ancestors()?;
    let metadata = fs::metadata(parent)?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.permissions().mode() & 0o077 != 0 {
        return Err(PreferenceError::InsecureFile);
    }
    Ok(())
}

fn staging_name(name: &str) -> bool {
    let Some((pid, sequence)) = name
        .strip_prefix(STAGING_PREFIX)
        .and_then(|name| name.strip_suffix(".tmp"))
        .and_then(|name| name.split_once('-'))
    else {
        return false;
    };
    pid.parse::<u32>()
        .is_ok_and(|value| value != 0 && value.to_string() == pid)
        && sequence
            .parse::<u64>()
            .is_ok_and(|value| value.to_string() == sequence)
}

/// Call only under the exclusive preference lease. Validate the whole bounded
/// inventory before any unlink; never follow links, recurse, parse partial data,
/// or sweep the generic settings/backup temporary-file namespace.
fn staged_orphans(path: &Path) -> Result<Vec<PathBuf>, PreferenceError> {
    let mut files = Vec::new();
    for (index, entry) in fs::read_dir(path.parent().unwrap())?.enumerate() {
        if index >= MAX_DIRECTORY_ENTRIES {
            return Err(PreferenceError::StagingLimit);
        }
        let entry = entry?;
        if entry.file_name().to_str().is_some_and(staging_name) {
            private_metadata(&fs::symlink_metadata(entry.path())?)?;
            files.push(entry.path());
            if files.len() > MAX_STAGING_FILES {
                return Err(PreferenceError::StagingLimit);
            }
        }
    }
    Ok(files)
}

fn remove_staging(files: &[PathBuf]) -> Result<(), PreferenceError> {
    for path in files {
        // Cooperating writers cannot change these entries while the lease is
        // held. Recheck unexpected changes; this is not hostile-same-UID CAS.
        private_metadata(&fs::symlink_metadata(path)?)?;
        fs::remove_file(path)?;
    }
    Ok(())
}

impl PreferenceStore {
    fn lease(path: &Path) -> Result<PreferenceLease, PreferenceError> {
        prepare_parent(path)?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path.parent().unwrap().join(LOCK_NAME))?;
        private_metadata(&lease.metadata()?)?;
        let owner_pid = unsafe { libc::getpid() };
        lease.try_lock().map_err(lock_error)?;
        // Construct the guard immediately after acquisition so every later
        // `?` in `open_at` releases the lock under the same ownership rule.
        Ok(PreferenceLease {
            file: lease,
            owner_pid,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<(Self, PreferenceCache), PreferenceError> {
        Self::open_at(path, Instant::now(), SystemTime::now())
    }

    pub fn open_at(
        path: impl AsRef<Path>,
        now: Instant,
        wall: SystemTime,
    ) -> Result<(Self, PreferenceCache), PreferenceError> {
        let path = path.as_ref();
        let lease = Self::lease(path)?;
        let baseline = read(path)?;
        let orphans = staged_orphans(path)?;
        remove_staging(&orphans)?;
        if !orphans.is_empty() {
            File::open(path.parent().unwrap())?.sync_all()?;
        }
        // A corrupt/unknown file prevents automatic replacement, not basic input.
        // The host can use a separate memory-only cache and report the error.
        let cache = match &baseline {
            Some(raw) => PreferenceCache::from_json_at(raw, now, wall)?,
            None => PreferenceCache::default(),
        };
        cache.snapshot_at(now, wall)?;
        Ok((
            Self {
                path: path.into(),
                baseline,
                owner: cache.snapshot_owner(),
                accepted_revision: cache.revision,
                cleared_revision: None,
                _lease: lease,
            },
            cache,
        ))
    }

    fn check(&self, snapshot: &PreferenceSnapshot) -> Result<(), PreferenceError> {
        if snapshot.owner != self.owner {
            return Err(PreferenceError::WrongCache);
        }
        if snapshot.revision < self.accepted_revision {
            return Err(PreferenceError::StaleSnapshot);
        }
        prepare_parent(&self.path)?;
        if read(&self.path)? != self.baseline {
            return Err(PreferenceError::Conflict);
        }
        Ok(())
    }

    fn sync_parent(&self) -> Result<(), PreferenceError> {
        File::open(self.path.parent().unwrap())?.sync_all()?;
        Ok(())
    }

    pub fn save(&mut self, snapshot: &PreferenceSnapshot) -> Result<(), PreferenceError> {
        self.save_if(snapshot, || true)
    }

    /// Serialize/stage/sync off-thread, then revalidate ownership before rename.
    /// A canceled prepared file is dropped, never published. The caller's
    /// predicate must not do I/O or block. Parent sync can still fail after rename.
    pub fn save_if(
        &mut self,
        snapshot: &PreferenceSnapshot,
        valid: impl FnOnce() -> bool,
    ) -> Result<(), PreferenceError> {
        self.check(snapshot)?;
        // Even an expiry-generated empty snapshot can be cleared. Its older
        // nonempty sibling has the same revision and must not recreate the file.
        if self
            .cleared_revision
            .is_some_and(|revision| snapshot.revision <= revision)
        {
            return Err(PreferenceError::StaleSnapshot);
        }
        let raw = snapshot.to_json()?;
        let staged = StagedFile::new_named(&self.path, raw.as_bytes(), STAGING_TAG)?;
        // Cooperating writers share the lease. Recheck after staging/fsync to
        // reject detected external edits; this is not CAS against a hostile editor.
        self.check(snapshot)?;
        if !valid() {
            return Err(PreferenceError::Cancelled);
        }
        staged.replace()?;
        self.baseline = Some(raw);
        self.accepted_revision = snapshot.revision;
        self.sync_parent()
    }

    /// The caller clears its RAM cache first, then sends that newer empty
    /// snapshot. Success means disk deletion is synced; older queued saves fail.
    pub fn clear(&mut self, snapshot: &PreferenceSnapshot) -> Result<(), PreferenceError> {
        self.clear_if(snapshot, || true)
    }

    pub fn clear_if(
        &mut self,
        snapshot: &PreferenceSnapshot,
        valid: impl FnOnce() -> bool,
    ) -> Result<(), PreferenceError> {
        if !snapshot.is_empty() {
            return Err(PreferenceError::NotEmpty);
        }
        self.check(snapshot)?;
        let orphans = staged_orphans(&self.path)?;
        if !valid() {
            return Err(PreferenceError::Cancelled);
        }
        remove_staging(&orphans)?;
        if self.baseline.is_some() {
            fs::remove_file(&self.path)?;
        }
        self.baseline = None;
        self.accepted_revision = snapshot.revision;
        self.cleared_revision = Some(snapshot.revision);
        self.sync_parent()
    }

    /// Explicit user reset can remove a corrupt/oversized document without
    /// parsing it. Only this exact owned private file may be deleted, under its
    /// exclusive lease; never follow links or delete a lock/other directory.
    /// Retain the lease and bind to the caller's now-empty cache for new choices.
    pub fn reset_if(
        path: impl AsRef<Path>,
        snapshot: &PreferenceSnapshot,
        valid: impl FnOnce() -> bool,
    ) -> Result<Self, PreferenceError> {
        if !snapshot.is_empty() {
            return Err(PreferenceError::NotEmpty);
        }
        let path = path.as_ref();
        let lease = Self::lease(path)?;
        let exists = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
        {
            Ok(file) => {
                private_metadata(&file.metadata()?)?;
                true
            }
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(error) => return Err(error.into()),
        };
        let orphans = staged_orphans(path)?;
        if !valid() {
            return Err(PreferenceError::Cancelled);
        }
        remove_staging(&orphans)?;
        if exists {
            fs::remove_file(path)?;
        }
        let store = Self {
            path: path.into(),
            baseline: None,
            owner: snapshot.owner,
            accepted_revision: snapshot.revision,
            cleared_revision: Some(snapshot.revision),
            _lease: lease,
        };
        store.sync_parent()?;
        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        os::unix::fs::DirBuilderExt,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "szpref-lease-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::DirBuilder::new().mode(0o700).create(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("create private test directory: {error}"),
                }
            }
        }

        fn path(&self) -> PathBuf {
            self.0.join(FILE_NAME)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            for name in [FILE_NAME, LOCK_NAME] {
                match fs::remove_file(self.0.join(name)) {
                    Ok(()) => {}
                    Err(error) if error.kind() == ErrorKind::NotFound => {}
                    Err(error) => panic!("remove test file: {error}"),
                }
            }
            fs::remove_dir(&self.0).unwrap();
        }
    }

    #[test]
    fn inherited_lease_drop_does_not_unlock_the_parent() {
        let directory = TestDirectory::new();
        let path = directory.path();
        let lease = PreferenceStore::lease(&path).unwrap();
        // In the child, only drop the guard (getpid + close) and _exit. Neither
        // allocators nor test machinery may run after fork in a threaded process.
        let child = unsafe { libc::fork() };
        assert!(child >= 0, "fork failed");
        if child == 0 {
            drop(lease);
            unsafe { libc::_exit(0) }
        }
        let mut status = 0;
        loop {
            let result = unsafe { libc::waitpid(child, &mut status, 0) };
            if result == child {
                break;
            }
            assert_eq!(result, -1);
            assert_eq!(
                std::io::Error::last_os_error().kind(),
                ErrorKind::Interrupted
            );
        }
        assert!(libc::WIFEXITED(status));
        assert_eq!(libc::WEXITSTATUS(status), 0);
        assert!(matches!(
            PreferenceStore::lease(&path),
            Err(PreferenceError::Busy)
        ));
        drop(lease);
        assert!(PreferenceStore::lease(&path).is_ok());
    }

    #[test]
    fn opening_errors_and_reset_cancellation_release_the_lease() {
        let directory = TestDirectory::new();
        let path = directory.path();
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap()
            .write_all(b"invalid json")
            .unwrap();
        assert!(matches!(
            PreferenceStore::open(&path),
            Err(PreferenceError::InvalidDocument)
        ));
        drop(PreferenceStore::lease(&path).unwrap());
        let snapshot = PreferenceCache::default()
            .snapshot_at(Instant::now(), SystemTime::now())
            .unwrap();
        assert!(matches!(
            PreferenceStore::reset_if(&path, &snapshot, || false),
            Err(PreferenceError::Cancelled)
        ));
        assert!(PreferenceStore::lease(&path).is_ok());
    }

    #[test]
    fn lock_errors_preserve_non_contention_io_kind() {
        assert_eq!(lock_error(TryLockError::WouldBlock), PreferenceError::Busy);
        assert_eq!(
            lock_error(TryLockError::Error(std::io::Error::from(
                ErrorKind::PermissionDenied
            ))),
            PreferenceError::Io(ErrorKind::PermissionDenied)
        );
        assert_eq!(
            lock_error(TryLockError::Error(std::io::Error::from(ErrorKind::Other))),
            PreferenceError::Io(ErrorKind::Other)
        );
    }
}

//! One bounded file worker. Input-side methods only move bounded RAM snapshots;
//! no file I/O, waiting for disk, or new thread per save/clear. A clear request is
//! explicitly accepted as an asynchronous global operation, not optimistic
//! success: callers must observe its exact receipt before claiming deletion.
use super::{
    PreferenceCache,
    persistence::{PreferenceError, PreferenceSnapshot},
    store::PreferenceStore,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClearStatus {
    pub id: u64,
    pub state: &'static str,
    pub error: Option<PreferenceError>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriterStatus {
    pub epoch: String,
    pub memory_only: bool,
    pub pending: bool,
    pub error: Option<PreferenceError>,
    pub clear: Option<ClearStatus>,
}

struct Save {
    snapshot: PreferenceSnapshot,
    sequence: u64,
    clear_id: u64,
}
struct Clear {
    snapshot: PreferenceSnapshot,
    id: u64,
    deadline: Instant,
}
struct Queue {
    save: Option<Save>,
    clear: Option<Clear>,
    sequence: u64,
    clear_id: u64,
    clear_revision: Option<u64>,
    private: bool,
    closing: bool,
    status: WriterStatus,
}
struct Shared {
    queue: Mutex<Queue>,
    changed: Condvar,
    #[cfg(test)]
    gate: Option<TestGate>,
}
#[cfg(test)]
type TestGate = Arc<dyn Fn(&str) + Send + Sync>;
struct Client {
    shared: Arc<Shared>,
    finished: Mutex<mpsc::Receiver<bool>>,
}
impl Drop for Client {
    fn drop(&mut self) {
        self.shared.queue.lock().unwrap().closing = true;
        self.shared.changed.notify_one();
    }
}

#[derive(Clone)]
pub struct PreferenceWriter(Arc<Client>);

impl PreferenceWriter {
    /// Call only before the input loop starts, on an owned startup path. A slow
    /// bootstrap fails closed within one second; its sole worker retains its
    /// lease until it exits, never installs late or publishes into another cache.
    pub fn start(
        path: PathBuf,
        timeout: Duration,
    ) -> Result<(Self, PreferenceCache), PreferenceError> {
        Self::start_inner(
            path,
            timeout,
            #[cfg(test)]
            None,
        )
    }

    fn start_inner(
        path: PathBuf,
        timeout: Duration,
        #[cfg(test)] gate: Option<TestGate>,
    ) -> Result<(Self, PreferenceCache), PreferenceError> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                save: None,
                clear: None,
                sequence: 0,
                clear_id: 0,
                clear_revision: None,
                private: false,
                closing: false,
                status: WriterStatus {
                    epoch: String::new(),
                    memory_only: true,
                    pending: false,
                    error: None,
                    clear: None,
                },
            }),
            changed: Condvar::new(),
            #[cfg(test)]
            gate,
        });
        let (opened_tx, opened_rx) = mpsc::sync_channel(1);
        let (finished_tx, finished_rx) = mpsc::channel();
        let writer = Self(Arc::new(Client {
            shared: shared.clone(),
            finished: Mutex::new(finished_rx),
        }));
        std::thread::Builder::new()
            .name("suzaku-preferences".into())
            .spawn(move || {
                let (store, cache, error) = match PreferenceStore::open(&path) {
                    Ok((store, cache)) => (Some(store), cache, None),
                    Err(error) => (None, PreferenceCache::default(), Some(error)),
                };
                {
                    let mut queue = shared.queue.lock().unwrap();
                    queue.status.memory_only = store.is_none();
                    queue.status.error = error;
                    let epoch = cache.fingerprint(&"suzaku-preference-writer-epoch-v1");
                    queue.status.epoch = format!("{:016x}{:016x}", epoch.0[0], epoch.0[1]);
                }
                #[cfg(test)]
                if let Some(gate) = &shared.gate {
                    gate("bootstrap");
                }
                if opened_tx.send(cache).is_ok() {
                    let clean = run(&shared, &path, store);
                    let _ = finished_tx.send(clean);
                }
            })?;
        let cache = opened_rx
            .recv_timeout(timeout.min(Duration::from_secs(1)))
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => PreferenceError::StartupTimeout,
                mpsc::RecvTimeoutError::Disconnected => {
                    PreferenceError::Io(std::io::ErrorKind::Other)
                }
            })?;
        Ok((writer, cache))
    }

    pub fn status(&self) -> WriterStatus {
        self.0.shared.queue.lock().unwrap().status.clone()
    }

    /// Coalesce, never block on file I/O. Clear has a separate priority slot and
    /// fence: a new post-clear choice must not cancel the user's clear request.
    pub fn schedule(&self, snapshot: PreferenceSnapshot) {
        let mut queue = self.0.shared.queue.lock().unwrap();
        if queue.closing
            || queue.private
            || queue
                .clear_revision
                .is_some_and(|revision| snapshot.revision() <= revision)
        {
            return;
        }
        queue.sequence = queue
            .sequence
            .checked_add(1)
            .expect("preference work exhausted");
        queue.save = Some(Save {
            snapshot,
            sequence: queue.sequence,
            clear_id: queue.clear_id,
        });
        queue.status.pending = true;
        self.0.shared.changed.notify_one();
    }

    pub fn set_private(&self, private: bool) {
        let mut queue = self.0.shared.queue.lock().unwrap();
        if queue.private != private {
            queue.private = private;
            if private {
                queue.sequence = queue
                    .sequence
                    .checked_add(1)
                    .expect("preference work exhausted");
                queue.save = None;
                queue.status.pending = queue
                    .status
                    .clear
                    .as_ref()
                    .is_some_and(|clear| clear.state == "pending");
            }
        }
    }

    /// RAM has already been cleared. Success here is acceptance only. A pending
    /// clear can be replaced by a newer clear, not by new typing. Deletion must
    /// begin within the supplied capped deadline, then be synced off-thread.
    pub fn clear(
        &self,
        snapshot: PreferenceSnapshot,
        timeout: Duration,
    ) -> Result<u64, PreferenceError> {
        if !snapshot.is_empty() {
            return Err(PreferenceError::NotEmpty);
        }
        let mut queue = self.0.shared.queue.lock().unwrap();
        if queue.closing {
            return Err(PreferenceError::Busy);
        }
        queue.clear_id = queue
            .clear_id
            .checked_add(1)
            .expect("preference clears exhausted");
        queue.sequence = queue
            .sequence
            .checked_add(1)
            .expect("preference work exhausted");
        queue.save = None;
        queue.clear_revision = Some(snapshot.revision());
        let id = queue.clear_id;
        queue.clear = Some(Clear {
            snapshot,
            id,
            deadline: Instant::now() + timeout.min(Duration::from_millis(600)),
        });
        queue.status.clear = Some(ClearStatus {
            id,
            state: "pending",
            error: None,
        });
        queue.status.pending = true;
        self.0.shared.changed.notify_one();
        Ok(id)
    }

    /// Call after the native input loop has stopped. Flush only the latest
    /// already-confirmed public snapshot; a timeout is unconfirmed, never a pass.
    /// Closing also prevents new work; no input-side method joins this worker.
    pub fn shutdown(&self, timeout: Duration) -> bool {
        self.0.shared.queue.lock().unwrap().closing = true;
        self.0.shared.changed.notify_one();
        self.0
            .finished
            .lock()
            .unwrap()
            .recv_timeout(timeout.min(Duration::from_secs(1)))
            == Ok(true)
    }
}

enum Work {
    Save(Save),
    Clear(Clear),
}

fn run(shared: &Shared, path: &Path, mut store: Option<PreferenceStore>) -> bool {
    loop {
        let work = {
            let mut queue = shared.queue.lock().unwrap();
            loop {
                if let Some(clear) = queue.clear.take() {
                    break Work::Clear(clear);
                }
                if let Some(save) = queue.save.take() {
                    break Work::Save(save);
                }
                if queue.closing {
                    return !queue.status.pending && queue.status.error.is_none();
                }
                queue = shared.changed.wait(queue).unwrap();
            }
        };
        #[cfg(test)]
        if let Some(gate) = &shared.gate {
            gate(match work {
                Work::Save(_) => "save",
                Work::Clear(_) => "clear",
            });
        }
        match work {
            Work::Save(save) => {
                let valid = || {
                    let queue = shared.queue.lock().unwrap();
                    !queue.private
                        && queue.sequence == save.sequence
                        && queue.clear_id == save.clear_id
                };
                let result = if !valid() {
                    Err(PreferenceError::Cancelled)
                } else if let Some(store) = store.as_mut() {
                    store.save_if(&save.snapshot, valid)
                } else {
                    Err(shared
                        .queue
                        .lock()
                        .unwrap()
                        .status
                        .error
                        .unwrap_or(PreferenceError::Busy))
                };
                let mut queue = shared.queue.lock().unwrap();
                if queue.sequence == save.sequence {
                    queue.status.pending = queue.clear.is_some();
                    if result != Err(PreferenceError::Cancelled) {
                        queue.status.error = result.err();
                    }
                }
            }
            Work::Clear(clear) => {
                let valid = || {
                    Instant::now() < clear.deadline
                        && shared.queue.lock().unwrap().clear_id == clear.id
                };
                let result = if let Some(store) = store.as_mut() {
                    store.clear_if(&clear.snapshot, valid)
                } else {
                    PreferenceStore::reset_if(path, &clear.snapshot, valid)
                        .map(|new| store = Some(new))
                };
                let mut queue = shared.queue.lock().unwrap();
                if queue.clear_id == clear.id {
                    queue.status.memory_only = store.is_none();
                    queue.status.pending = queue.save.is_some();
                    queue.status.error = result.err();
                    queue.status.clear = Some(ClearStatus {
                        id: clear.id,
                        state: if result.is_ok() { "complete" } else { "failed" },
                        error: result.err(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::store::FILE_NAME;
    use std::{
        fs,
        io::Write,
        os::unix::fs::{DirBuilderExt, OpenOptionsExt},
        sync::atomic::{AtomicBool, AtomicU64, Ordering},
        time::SystemTime,
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Owned(PathBuf);
    impl Owned {
        fn new() -> Self {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "szpref-worker-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::DirBuilder::new().mode(0o700).create(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("owned fixture: {error}"),
                }
            }
        }
        fn path(&self) -> PathBuf {
            self.0.join(FILE_NAME)
        }
        fn write(&self, bytes: &[u8]) {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.path())
                .unwrap()
                .write_all(bytes)
                .unwrap();
        }
    }
    impl Drop for Owned {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn snap(cache: &PreferenceCache) -> PreferenceSnapshot {
        cache
            .snapshot_at(Instant::now(), SystemTime::now())
            .unwrap()
    }
    fn wait(predicate: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while !predicate() {
            assert!(Instant::now() < deadline, "owned worker gate deadline");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    struct Barrier {
        entered: mpsc::Receiver<()>,
        release: mpsc::Sender<()>,
    }
    fn gate(kind: &'static str) -> (TestGate, Barrier) {
        let (entered_tx, entered) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        let armed = AtomicBool::new(true);
        (
            Arc::new(move |operation| {
                if operation == kind && armed.swap(false, Ordering::AcqRel) {
                    entered_tx.send(()).unwrap();
                    release_rx
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(1))
                        .unwrap();
                }
            }),
            Barrier { entered, release },
        )
    }
    fn start(owned: &Owned, gate: Option<TestGate>) -> (PreferenceWriter, PreferenceCache) {
        PreferenceWriter::start_inner(owned.path(), Duration::from_millis(500), gate).unwrap()
    }
    fn choose(cache: &mut PreferenceCache) {
        cache.record(cache.key("en", "hel", "hello").unwrap(), Instant::now());
    }
    fn frequency(owned: &Owned) -> f32 {
        let (_store, cache) = PreferenceStore::open(owned.path()).unwrap();
        cache.bonus(cache.key("en", "hel", "hello").unwrap(), Instant::now())
    }

    #[test]
    fn held_save_coalesces_to_latest_snapshot_without_an_input_side_wait() {
        let owned = Owned::new();
        let (gate, barrier) = gate("save");
        let (writer, mut cache) = start(&owned, Some(gate));
        assert!(!writer.status().memory_only);
        assert_eq!(writer.status().epoch.len(), 32);
        choose(&mut cache);
        writer.schedule(snap(&cache));
        barrier
            .entered
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        let started = Instant::now();
        for _ in 0..20 {
            choose(&mut cache);
            writer.schedule(snap(&cache));
        }
        assert!(started.elapsed() < Duration::from_millis(200));
        assert!(writer.0.shared.queue.lock().unwrap().save.is_some());
        assert!(writer.status().pending);
        assert!(!owned.path().exists());
        barrier.release.send(()).unwrap();
        wait(|| !writer.status().pending);
        assert_eq!(writer.status().error, None);
        assert!(writer.shutdown(Duration::from_millis(500)));
        assert!(frequency(&owned) > 23.9);
    }

    #[test]
    fn clear_cancels_held_save_and_prevents_old_snapshot_resurrection() {
        let owned = Owned::new();
        let (gate, barrier) = gate("save");
        let (writer, mut cache) = start(&owned, Some(gate));
        choose(&mut cache);
        let old = snap(&cache);
        writer.schedule(old.clone());
        barrier
            .entered
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        cache.clear();
        let id = writer
            .clear(snap(&cache), Duration::from_millis(500))
            .unwrap();
        assert_eq!(writer.status().clear.as_ref().unwrap().state, "pending");
        barrier.release.send(()).unwrap();
        wait(|| writer.status().clear.as_ref().unwrap().state == "complete");
        assert_eq!(writer.status().clear.as_ref().unwrap().id, id);
        assert!(!owned.path().exists());
        writer.schedule(old);
        writer.schedule(snap(&cache));
        assert!(!writer.status().pending);
        choose(&mut cache);
        writer.schedule(snap(&cache));
        assert!(writer.shutdown(Duration::from_millis(500)));
        assert!(frequency(&owned) > 2.9);
        assert!(frequency(&owned) <= 3.0);
    }

    #[test]
    fn post_clear_typing_never_cancels_the_clear_receipt() {
        let owned = Owned::new();
        let (gate, barrier) = gate("clear");
        let (writer, mut cache) = start(&owned, Some(gate));
        cache.clear();
        let id = writer
            .clear(snap(&cache), Duration::from_millis(500))
            .unwrap();
        barrier
            .entered
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        choose(&mut cache);
        writer.schedule(snap(&cache));
        barrier.release.send(()).unwrap();
        wait(|| writer.status().clear.as_ref().unwrap().state == "complete");
        assert_eq!(writer.status().clear.as_ref().unwrap().id, id);
        assert!(writer.shutdown(Duration::from_millis(500)));
        assert!(frequency(&owned) > 2.9);
    }

    #[test]
    fn privacy_cancels_a_public_save_then_resume_accepts_a_fresh_capture() {
        let owned = Owned::new();
        let (gate, barrier) = gate("save");
        let (writer, mut cache) = start(&owned, Some(gate));
        choose(&mut cache);
        writer.schedule(snap(&cache));
        barrier
            .entered
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        writer.set_private(true);
        writer.schedule(snap(&cache));
        assert!(writer.0.shared.queue.lock().unwrap().save.is_none());
        barrier.release.send(()).unwrap();
        // Observe the same worker advancing past the canceled save through an
        // explicit private-context global clear; no private text is exported.
        cache.clear();
        writer
            .clear(snap(&cache), Duration::from_millis(500))
            .unwrap();
        wait(|| writer.status().clear.as_ref().unwrap().state == "complete");
        assert!(!owned.path().exists());
        writer.set_private(false);
        choose(&mut cache);
        writer.schedule(snap(&cache));
        assert!(writer.shutdown(Duration::from_millis(500)));
        assert!(frequency(&owned) > 2.9);
    }

    #[test]
    fn expired_clear_is_failed_without_deletion_and_new_clear_can_confirm() {
        let owned = Owned::new();
        let (writer, mut cache) = start(&owned, None);
        choose(&mut cache);
        writer.schedule(snap(&cache));
        wait(|| !writer.status().pending);
        let original = fs::read(owned.path()).unwrap();
        cache.clear();
        writer.clear(snap(&cache), Duration::ZERO).unwrap();
        wait(|| writer.status().clear.as_ref().unwrap().state == "failed");
        assert_eq!(
            writer.status().clear.as_ref().unwrap().error,
            Some(PreferenceError::Cancelled)
        );
        assert_eq!(fs::read(owned.path()).unwrap(), original);
        cache.clear();
        writer
            .clear(snap(&cache), Duration::from_millis(500))
            .unwrap();
        assert!(writer.shutdown(Duration::from_millis(500)));
        assert!(!owned.path().exists());
    }

    #[test]
    fn newer_clear_replaces_only_the_older_clear_not_the_worker() {
        let owned = Owned::new();
        let (gate, barrier) = gate("clear");
        let (writer, mut cache) = start(&owned, Some(gate));
        cache.clear();
        let old = writer
            .clear(snap(&cache), Duration::from_millis(500))
            .unwrap();
        barrier
            .entered
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        cache.clear();
        let new = writer
            .clear(snap(&cache), Duration::from_millis(500))
            .unwrap();
        assert!(new > old);
        barrier.release.send(()).unwrap();
        assert!(writer.shutdown(Duration::from_millis(500)));
        let receipt = writer.status().clear.unwrap();
        assert_eq!(
            (receipt.id, receipt.state, receipt.error),
            (new, "complete", None)
        );
    }

    #[test]
    fn bounded_shutdown_reports_unconfirmed_held_io_and_rejects_new_work() {
        let owned = Owned::new();
        let (gate, barrier) = gate("save");
        let (writer, mut cache) = start(&owned, Some(gate));
        choose(&mut cache);
        writer.schedule(snap(&cache));
        barrier
            .entered
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        assert!(!writer.shutdown(Duration::from_millis(10)));
        choose(&mut cache);
        writer.schedule(snap(&cache));
        assert!(writer.0.shared.queue.lock().unwrap().save.is_none());
        barrier.release.send(()).unwrap();
        // This is observation of the already-closing worker, not a restart or a
        // retry of any file operation. Only its originally accepted save remains.
        assert!(writer.shutdown(Duration::from_millis(500)));
        assert!(frequency(&owned) > 2.9 && frequency(&owned) <= 3.0);
    }

    #[test]
    fn timed_out_bootstrap_keeps_its_lease_until_the_single_worker_exits() {
        let owned = Owned::new();
        let (mut store, mut cache) = PreferenceStore::open(owned.path()).unwrap();
        choose(&mut cache);
        store.save(&snap(&cache)).unwrap();
        drop(store);
        let original = fs::read(owned.path()).unwrap();
        let (gate, barrier) = gate("bootstrap");
        let started = Instant::now();
        assert_eq!(
            PreferenceWriter::start_inner(owned.path(), Duration::from_millis(10), Some(gate))
                .err(),
            Some(PreferenceError::StartupTimeout)
        );
        assert!(started.elapsed() < Duration::from_millis(200));
        barrier
            .entered
            .recv_timeout(Duration::from_millis(500))
            .unwrap();
        assert_eq!(
            PreferenceStore::open(owned.path()).err(),
            Some(PreferenceError::Busy)
        );
        barrier.release.send(()).unwrap();
        // Observe only lease availability, never start a replacement worker or
        // install a cache after the original startup budget has expired.
        let mut reopened = None;
        let deadline = Instant::now() + Duration::from_millis(500);
        while reopened.is_none() {
            match PreferenceStore::open(owned.path()) {
                Ok(result) => reopened = Some(result),
                Err(PreferenceError::Busy) => {
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("bootstrap lease observation: {error}"),
            }
        }
        assert_eq!(fs::read(owned.path()).unwrap(), original);
        assert_eq!(reopened.unwrap().1.len(), 1);
    }

    #[test]
    fn corrupt_and_oversized_files_stay_untouched_until_explicit_private_reset() {
        for bytes in [
            b"broken synthetic document".to_vec(),
            vec![b'x'; crate::preferences::persistence::MAX_FILE_BYTES + 1],
        ] {
            let owned = Owned::new();
            owned.write(&bytes);
            let (writer, mut cache) = start(&owned, None);
            assert!(writer.status().memory_only);
            assert!(writer.status().error.is_some());
            choose(&mut cache);
            writer.schedule(snap(&cache));
            wait(|| !writer.status().pending);
            assert_eq!(fs::read(owned.path()).unwrap(), bytes);
            cache.clear();
            writer
                .clear(snap(&cache), Duration::from_millis(500))
                .unwrap();
            wait(|| writer.status().clear.as_ref().unwrap().state == "complete");
            assert!(!owned.path().exists());
            assert!(!writer.status().memory_only);
            choose(&mut cache);
            writer.schedule(snap(&cache));
            assert!(writer.shutdown(Duration::from_millis(500)));
            assert!(frequency(&owned) > 2.9);
        }
    }

    #[test]
    fn foreign_writer_or_link_cannot_be_erased_by_a_clear_request() {
        let owned = Owned::new();
        let (first, _) = start(&owned, None);
        let (second, mut cache) = start(&owned, None);
        assert_eq!(second.status().error, Some(PreferenceError::Busy));
        cache.clear();
        second
            .clear(snap(&cache), Duration::from_millis(500))
            .unwrap();
        wait(|| second.status().clear.as_ref().unwrap().state == "failed");
        assert_eq!(second.status().error, Some(PreferenceError::Busy));
        assert!(!second.shutdown(Duration::from_millis(500)));
        assert!(first.shutdown(Duration::from_millis(500)));
        drop(first);
        drop(second);
        let sentinel = owned.0.join("keep");
        fs::write(&sentinel, b"synthetic sentinel").unwrap();
        std::os::unix::fs::symlink(&sentinel, owned.path()).unwrap();
        let (writer, mut cache) = start(&owned, None);
        cache.clear();
        writer
            .clear(snap(&cache), Duration::from_millis(500))
            .unwrap();
        wait(|| writer.status().clear.as_ref().unwrap().state == "failed");
        assert!(!writer.shutdown(Duration::from_millis(500)));
        assert!(
            fs::symlink_metadata(owned.path())
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(sentinel).unwrap(), b"synthetic sentinel");
    }
}

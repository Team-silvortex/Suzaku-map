#![cfg(target_os = "linux")]

//! Exercise the real preference writer with a child stopped between fork and
//! exec. The child uses only async-signal-safe syscalls until it execs.
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::{fd::FromRawFd, unix::fs::DirBuilderExt},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use suzaku_map::preferences::{
    persistence::PreferenceError,
    store::{FILE_NAME, PreferenceStore},
    worker::PreferenceWriter,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct PrivateDir(PathBuf);

impl PrivateDir {
    fn new() -> Self {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "suzaku-pref-fork-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not create private fixture: {error}"),
            }
        }
        panic!("could not reserve private fixture");
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let lock_path = self.0.join(".suzaku-preferences.lock");
        match fs::remove_file(lock_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("remove private test lock: {error}"),
        }
        fs::remove_dir(&self.0).expect("remove private test fixture");
    }
}

struct PausedChild {
    pid: libc::pid_t,
    release: File,
}

impl PausedChild {
    fn spawn() -> Self {
        let mut ready = [-1; 2];
        let mut release = [-1; 2];
        // O_CLOEXEC prevents the synchronization pipes from leaking into exec.
        assert_eq!(
            unsafe { libc::pipe2(ready.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );
        let mut ready_read = unsafe { File::from_raw_fd(ready[0]) };
        let ready_write = unsafe { File::from_raw_fd(ready[1]) };
        assert_eq!(
            unsafe { libc::pipe2(release.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );
        let release_read = unsafe { File::from_raw_fd(release[0]) };
        let release_write = unsafe { File::from_raw_fd(release[1]) };

        let program = c"/bin/true";
        let args = [program.as_ptr(), std::ptr::null()];
        let env = [std::ptr::null()];
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0, "fork failed: {}", std::io::Error::last_os_error());
        if pid == 0 {
            // No Rust allocation, locks, destructors, or test assertions here.
            unsafe {
                libc::close(ready[0]);
                libc::close(release[1]);
                let byte = [1_u8];
                if libc::write(ready[1], byte.as_ptr().cast(), 1) != 1 {
                    libc::_exit(120);
                }
                let mut response = [0_u8];
                if libc::read(release[0], response.as_mut_ptr().cast(), 1) != 1 {
                    libc::_exit(121);
                }
                libc::execve(program.as_ptr(), args.as_ptr(), env.as_ptr());
                libc::_exit(122);
            }
        }
        drop(ready_write);
        drop(release_read);
        let child = Self {
            pid,
            release: release_write,
        };
        let mut poll_fd = libc::pollfd {
            fd: ready[0],
            events: libc::POLLIN,
            revents: 0,
        };
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "child did not signal readiness");
            let result = unsafe { libc::poll(&mut poll_fd, 1, remaining.as_millis() as i32 + 1) };
            if result == 1 {
                break;
            }
            if result < 0
                && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
            {
                continue;
            }
            panic!("child did not signal readiness: poll returned {result}");
        }
        let mut byte = [0_u8];
        ready_read.read_exact(&mut byte).expect("child readiness");
        assert_eq!(byte, [1]);
        child
    }

    fn exec_and_wait(mut self) {
        self.release.write_all(&[1]).expect("release child");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let mut status = 0;
            let result = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
            if result == self.pid {
                self.pid = 0;
                assert!(libc::WIFEXITED(status));
                assert_eq!(libc::WEXITSTATUS(status), 0, "child exec failed");
                return;
            }
            if result < 0
                && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
            {
                assert!(Instant::now() < deadline, "child did not finish");
                continue;
            }
            assert_eq!(
                result,
                0,
                "waitpid failed: {}",
                std::io::Error::last_os_error()
            );
            assert!(Instant::now() < deadline, "child did not finish");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for PausedChild {
    fn drop(&mut self) {
        if self.pid > 0 {
            unsafe {
                libc::kill(self.pid, libc::SIGKILL);
                libc::waitpid(self.pid, std::ptr::null_mut(), 0);
            }
        }
    }
}

#[test]
fn writer_releases_lease_while_child_is_still_before_exec() {
    let directory = PrivateDir::new();
    let path = directory.0.join(FILE_NAME);
    let (writer, _) = PreferenceWriter::start(path.clone(), Duration::from_secs(1)).unwrap();
    assert!(
        !writer.status().memory_only,
        "writer must actually hold the lease"
    );
    let child = PausedChild::spawn();

    assert!(matches!(
        PreferenceStore::open(&path),
        Err(PreferenceError::Busy)
    ));
    assert!(
        writer.shutdown(Duration::from_secs(1)),
        "writer shutdown timed out"
    );
    let (store, _) = PreferenceStore::open(&path)
        .expect("writer shutdown should release the lease before inherited fd exec");
    assert!(matches!(
        PreferenceStore::open(&path),
        Err(PreferenceError::Busy)
    ));
    drop(store);
    child.exec_and_wait();
    assert!(PreferenceStore::open(&path).is_ok());
}

#[test]
fn child_exec_cannot_release_a_live_writers_lease() {
    let directory = PrivateDir::new();
    let path = directory.0.join(FILE_NAME);
    let (writer, _) = PreferenceWriter::start(path.clone(), Duration::from_secs(1)).unwrap();
    assert!(!writer.status().memory_only);
    let child = PausedChild::spawn();
    child.exec_and_wait();

    assert!(matches!(
        PreferenceStore::open(&path),
        Err(PreferenceError::Busy)
    ));
    assert!(writer.shutdown(Duration::from_secs(1)));
    assert!(PreferenceStore::open(&path).is_ok());
}

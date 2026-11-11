#![cfg(target_os = "linux")]

//! Real terminal tests use private pseudo-terminals, files and synthetic targets.
//! No inherited credential environment, desktop service or model is involved.
use std::{
    fs::{self, File},
    io::{Read, Write},
    net::TcpListener,
    os::{fd::AsRawFd, fd::FromRawFd, unix::fs::DirBuilderExt},
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const MAX_OUTPUT: usize = 65_536;
const COMMAND_DEADLINE: Duration = Duration::from_secs(2);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "suzaku-model-setup-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("private setup fixture: {error}"),
            }
        }
        panic!("could not reserve private setup fixture");
    }

    fn settings(&self) -> PathBuf {
        self.0.join("settings.json")
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"));
        command
            .args(["model", "setup"])
            .env_clear()
            .current_dir(&self.0)
            .env("HOME", self.0.join("home"))
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env("XDG_STATE_HOME", self.0.join("state"))
            .env("XDG_CACHE_HOME", self.0.join("cache"))
            .env("SUZAKU_IME_CONFIG", self.settings())
            .env("LC_ALL", "C")
            .env("TERM", "dumb");
        command
    }

    fn assert_absent(&self) {
        assert_eq!(fs::read_dir(&self.0).unwrap().count(), 0);
    }

    fn assert_bytes(&self, expected: &[u8]) {
        assert_eq!(fs::read(self.settings()).unwrap(), expected);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove private setup fixture");
    }
}

struct Terminal {
    child: Option<Child>,
    master: File,
    transcript: Vec<u8>,
    consumed: usize,
    deadline: Instant,
}

impl Terminal {
    fn start(fixture: &Fixture, stdin_tty: bool, stdout_tty: bool) -> Self {
        let deadline = Instant::now() + COMMAND_DEADLINE;
        // O_CLOEXEC is atomic, avoiding PTY leakage to concurrent test children.
        let raw_master =
            unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC) };
        assert!(
            raw_master >= 0,
            "posix_openpt: {}",
            std::io::Error::last_os_error()
        );
        // SAFETY: each successful raw descriptor is owned exactly once.
        let master = unsafe { File::from_raw_fd(raw_master) };
        assert_eq!(unsafe { libc::grantpt(raw_master) }, 0);
        assert_eq!(unsafe { libc::unlockpt(raw_master) }, 0);
        let mut name = [0 as libc::c_char; 128];
        assert_eq!(
            unsafe { libc::ptsname_r(raw_master, name.as_mut_ptr(), name.len()) },
            0
        );
        let raw_slave = unsafe {
            libc::open(
                name.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC,
            )
        };
        assert!(
            raw_slave >= 0,
            "open PTY slave: {}",
            std::io::Error::last_os_error()
        );
        let slave = unsafe { File::from_raw_fd(raw_slave) };
        let mut terminal = std::mem::MaybeUninit::<libc::termios>::uninit();
        assert_eq!(
            unsafe { libc::tcgetattr(raw_slave, terminal.as_mut_ptr()) },
            0
        );
        let mut terminal = unsafe { terminal.assume_init() };
        // Keep canonical input for real VEOF behavior, but no synthetic input echo.
        terminal.c_lflag |= libc::ICANON;
        terminal.c_lflag &= !(libc::ECHO | libc::ECHONL);
        terminal.c_cc[libc::VEOF] = 4;
        assert_eq!(
            unsafe { libc::tcsetattr(raw_slave, libc::TCSANOW, &terminal) },
            0
        );
        let flags = unsafe { libc::fcntl(raw_master, libc::F_GETFL) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(raw_master, libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );

        let mut command = fixture.command();
        command
            .stdin(if stdin_tty {
                Stdio::from(slave.try_clone().unwrap())
            } else {
                Stdio::null()
            })
            .stdout(if stdout_tty {
                Stdio::from(slave.try_clone().unwrap())
            } else {
                Stdio::null()
            })
            .stderr(Stdio::from(slave));
        let child = command.spawn().expect("spawn private terminal setup");
        Self {
            child: Some(child),
            master,
            transcript: Vec::new(),
            consumed: 0,
            deadline,
        }
    }

    fn read_available(&mut self) {
        let mut buffer = [0_u8; 4096];
        loop {
            match self.master.read(&mut buffer) {
                Ok(0) => return,
                Ok(count) => {
                    assert!(
                        self.transcript.len() + count <= MAX_OUTPUT,
                        "setup output exceeded 64 KiB"
                    );
                    self.transcript.extend_from_slice(&buffer[..count]);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.raw_os_error() == Some(libc::EIO) =>
                {
                    return;
                }
                Err(error) => panic!("read private terminal: {error}"),
            }
        }
    }

    fn poll(&self) {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "setup exceeded unchanged two-second deadline"
        );
        let mut fd = libc::pollfd {
            fd: self.master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let timeout = remaining.as_millis().clamp(1, 10) as i32;
        let result = unsafe { libc::poll(&mut fd, 1, timeout) };
        assert!(
            result >= 0
                || std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
        );
    }

    fn expect(&mut self, prompt: &str) {
        loop {
            self.read_available();
            if let Some(offset) = self.transcript[self.consumed..]
                .windows(prompt.len())
                .position(|part| part == prompt.as_bytes())
            {
                self.consumed += offset + prompt.len();
                return;
            }
            assert!(
                self.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "setup exited before {prompt:?}: {}",
                String::from_utf8_lossy(&self.transcript)
            );
            self.poll();
        }
    }

    fn line(&mut self, value: &str) {
        self.master
            .write_all(value.as_bytes())
            .expect("terminal answer");
        self.master
            .write_all(b"\n")
            .expect("terminal answer newline");
    }

    fn eof(&mut self) {
        self.master.write_all(&[4]).expect("terminal VEOF");
    }

    fn finish(mut self) -> (ExitStatus, String) {
        loop {
            self.read_available();
            if let Some(status) = self.child.as_mut().unwrap().try_wait().unwrap() {
                self.read_available();
                self.child.take();
                return (status, String::from_utf8(self.transcript.clone()).unwrap());
            }
            self.poll();
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct NoRequests(TcpListener);

impl NoRequests {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        Self(listener)
    }

    fn base(&self) -> String {
        format!("https://{}/v1", self.0.local_addr().unwrap())
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base())
    }

    fn assert_idle(&self) {
        match self.0.accept() {
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            _ => panic!("offline setup attempted a model connection"),
        }
    }
}

const PROMPTS: [&str; 8] = [
    "Scope [",
    "Address mode [",
    "Protocol [",
    "Endpoint (",
    "Model ID (",
    "Credential environment name (",
    "Timeout ms (",
    "Type save to save; anything else cancels: ",
];

fn baseline(fixture: &Fixture, endpoint: &str, protocol: &str) -> Vec<u8> {
    let value = serde_json::json!({
        "language": "ja",
        "llm_enabled": true,
        "llm_scope": "cloud",
        "llm_protocol": protocol,
        "llm_cloud_consent": true,
        "llm_endpoint": endpoint,
        "llm_model": "synthetic-baseline-model",
        "llm_api_key_env": "SUZAKU_SYNTHETIC_SETUP_KEY",
        "llm_timeout_ms": 1800,
        "llm_temperature_tenths": 8
    });
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(fixture.settings(), &bytes).unwrap();
    bytes
}

fn unchanged(fixture: &Fixture, before: Option<&[u8]>) {
    if let Some(before) = before {
        fixture.assert_bytes(before);
    } else {
        fixture.assert_absent();
    }
}

/// Reach a real, flushed prompt before answering; never pre-feed the dialogue.
fn fill(terminal: &mut Terminal, fixture: &Fixture, before: Option<&[u8]>, answers: &[&str]) {
    for (index, answer) in answers.iter().enumerate() {
        let prompt = if index == 3 && answers.get(1) == Some(&"base") {
            "API base ("
        } else {
            PROMPTS[index]
        };
        terminal.expect(prompt);
        unchanged(fixture, before);
        terminal.line(answer);
    }
}

fn assert_cancelled(terminal: Terminal) {
    let (status, output) = terminal.finish();
    assert!(status.success(), "cancel failed: {output}");
    assert!(output.to_ascii_lowercase().contains("cancel"), "{output}");
}

#[test]
fn setup_requires_both_real_terminal_streams_before_reading_configuration() {
    for existing in [false, true] {
        let fixture = Fixture::new();
        let broken = b"broken JSON must never be read before the terminal check";
        if existing {
            fs::write(fixture.settings(), broken).unwrap();
        }
        for (stdin_tty, stdout_tty) in [(false, true), (true, false)] {
            let terminal = Terminal::start(&fixture, stdin_tty, stdout_tty);
            let (status, output) = terminal.finish();
            assert!(!status.success(), "non-terminal setup accepted");
            assert!(output.to_ascii_lowercase().contains("terminal"), "{output}");
            assert!(!output.contains("Scope ["));
            unchanged(&fixture, existing.then_some(broken.as_slice()));
        }
    }
}

#[test]
fn cancellation_at_every_terminal_prompt_never_creates_or_replaces_settings() {
    let no_requests = NoRequests::new();
    let endpoint = no_requests.endpoint();
    for existing in [false, true] {
        for cancelled_prompt in 0..PROMPTS.len() {
            let fixture = Fixture::new();
            let before = existing.then(|| baseline(&fixture, &endpoint, "chat-completions"));
            let mut terminal = Terminal::start(&fixture, true, true);
            let answers = [
                "cloud",
                "endpoint",
                "chat-completions",
                endpoint.as_str(),
                "synthetic-model",
                "SUZAKU_SYNTHETIC_SETUP_KEY",
                "1800",
            ];
            fill(
                &mut terminal,
                &fixture,
                before.as_deref(),
                &answers[..cancelled_prompt],
            );
            terminal.expect(PROMPTS[cancelled_prompt]);
            unchanged(&fixture, before.as_deref());
            terminal.line(":cancel");
            assert_cancelled(terminal);
            unchanged(&fixture, before.as_deref());
        }
    }
    no_requests.assert_idle();
}

#[test]
fn terminal_eof_at_every_prompt_and_unterminated_partial_lines_cancel_without_writes() {
    let no_requests = NoRequests::new();
    let endpoint = no_requests.endpoint();
    for (eof_prompt, prompt) in PROMPTS.iter().enumerate() {
        let fixture = Fixture::new();
        let before = baseline(&fixture, &endpoint, "chat-completions");
        let mut terminal = Terminal::start(&fixture, true, true);
        fill(
            &mut terminal,
            &fixture,
            Some(&before),
            &vec![""; eof_prompt],
        );
        terminal.expect(prompt);
        fixture.assert_bytes(&before);
        terminal.eof();
        assert_cancelled(terminal);
        fixture.assert_bytes(&before);
    }

    let fixture = Fixture::new();
    let mut terminal = Terminal::start(&fixture, true, true);
    terminal.expect(PROMPTS[0]);
    terminal.master.write_all(b"cloud").unwrap();
    // First VEOF releases the partial canonical line; the second delivers EOF.
    terminal.eof();
    terminal.eof();
    assert_cancelled(terminal);
    fixture.assert_absent();

    let fixture = Fixture::new();
    let before = baseline(&fixture, &endpoint, "chat-completions");
    let mut terminal = Terminal::start(&fixture, true, true);
    fill(&mut terminal, &fixture, Some(&before), &[""; 7]);
    terminal.expect(PROMPTS[7]);
    terminal.master.write_all(b"save").unwrap();
    terminal.eof();
    terminal.eof();
    assert_cancelled(terminal);
    fixture.assert_bytes(&before);
    no_requests.assert_idle();
}

#[test]
fn confirmed_setup_preserves_protocol_and_other_settings_but_revokes_cloud_consent() {
    let no_requests = NoRequests::new();
    for protocol in [
        "openai-compatible",
        "chat-completions",
        "chat-completions-low",
    ] {
        let fixture = Fixture::new();
        let before = baseline(&fixture, &no_requests.endpoint(), protocol);
        let mut terminal = Terminal::start(&fixture, true, true);
        fill(&mut terminal, &fixture, Some(&before), &[""; 7]);
        terminal.expect(PROMPTS[7]);
        fixture.assert_bytes(&before);
        terminal.line("save");
        let (status, output) = terminal.finish();
        assert!(status.success(), "confirmed setup: {output}");
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture.settings()).unwrap()).unwrap();
        assert_eq!(value["llm_scope"], "cloud");
        assert_eq!(value["llm_protocol"], protocol);
        assert_eq!(value["llm_endpoint"], no_requests.endpoint());
        assert_eq!(value["llm_model"], "synthetic-baseline-model");
        assert_eq!(value["llm_api_key_env"], "SUZAKU_SYNTHETIC_SETUP_KEY");
        assert_eq!(value["llm_cloud_consent"], false);
        assert_eq!(value["llm_enabled"], true);
        assert_eq!(value["language"], "ja");
        assert_eq!(value["llm_timeout_ms"], 1800);
        assert_eq!(value["llm_temperature_tenths"], 8);
    }
    no_requests.assert_idle();
}

#[test]
fn confirmed_base_setup_creates_only_the_explicit_offline_configuration() {
    let fixture = Fixture::new();
    let no_requests = NoRequests::new();
    let base = no_requests.base();
    let mut terminal = Terminal::start(&fixture, true, true);
    fill(
        &mut terminal,
        &fixture,
        None,
        &[
            "cloud",
            "base",
            "chat-completions",
            &base,
            "synthetic-new-model",
            "SUZAKU_SYNTHETIC_SETUP_KEY",
            "1600",
        ],
    );
    terminal.expect(PROMPTS[7]);
    fixture.assert_absent();
    terminal.line("save");
    let (status, output) = terminal.finish();
    assert!(status.success(), "new setup: {output}");
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.settings()).unwrap()).unwrap();
    assert_eq!(value["llm_endpoint"], no_requests.endpoint());
    assert_eq!(value["llm_protocol"], "chat-completions");
    assert_eq!(value["llm_model"], "synthetic-new-model");
    assert_eq!(value["llm_api_key_env"], "SUZAKU_SYNTHETIC_SETUP_KEY");
    assert_eq!(value["llm_cloud_consent"], false);
    assert_eq!(value["llm_enabled"], false);
    assert_eq!(value["llm_timeout_ms"], 1600);
    no_requests.assert_idle();
}

#[test]
fn stale_confirmation_preserves_the_external_writers_exact_configuration() {
    let fixture = Fixture::new();
    let no_requests = NoRequests::new();
    let before = baseline(&fixture, &no_requests.endpoint(), "chat-completions");
    let mut terminal = Terminal::start(&fixture, true, true);
    fill(&mut terminal, &fixture, Some(&before), &[""; 7]);
    terminal.expect(PROMPTS[7]);
    fixture.assert_bytes(&before);
    let mut external: serde_json::Value = serde_json::from_slice(&before).unwrap();
    external["llm_model"] = "synthetic-external-writer".into();
    external["llm_enabled"] = false.into();
    external["llm_cloud_consent"] = false.into();
    let external_bytes = serde_json::to_vec_pretty(&external).unwrap();
    fs::write(fixture.settings(), &external_bytes).unwrap();
    terminal.line("save");
    let (status, output) = terminal.finish();
    assert!(
        !status.success(),
        "stale setup overwrote a changed configuration: {output}"
    );
    fixture.assert_bytes(&external_bytes);
    no_requests.assert_idle();
}

#[test]
fn changing_cloud_to_local_resets_scope_specific_defaults_and_preserves_llm_state() {
    let fixture = Fixture::new();
    let no_requests = NoRequests::new();
    let before = baseline(&fixture, &no_requests.endpoint(), "chat-completions");
    let mut terminal = Terminal::start(&fixture, true, true);
    fill(
        &mut terminal,
        &fixture,
        Some(&before),
        &["local", "", "", "", "", "", ""],
    );
    terminal.expect(PROMPTS[7]);
    fixture.assert_bytes(&before);
    terminal.line("save");
    let (status, output) = terminal.finish();
    assert!(status.success(), "cloud to local setup: {output}");
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.settings()).unwrap()).unwrap();
    assert_eq!(value["llm_scope"], "local");
    assert_eq!(value["llm_protocol"], "auto");
    assert_eq!(
        value["llm_endpoint"],
        suzaku_map::model::DEFAULT_LOCAL_ENDPOINT
    );
    assert_eq!(value["llm_model"], suzaku_map::model::DEFAULT_MODEL);
    assert_eq!(value["llm_api_key_env"], serde_json::Value::Null);
    assert_eq!(value["llm_cloud_consent"], false);
    assert_eq!(value["llm_enabled"], true);
    assert_eq!(value["language"], "ja");
    assert_eq!(value["llm_timeout_ms"], 1800);
    no_requests.assert_idle();
}

#[test]
fn changing_local_to_cloud_requires_a_new_address_and_model_instead_of_local_defaults() {
    let no_requests = NoRequests::new();
    let endpoint = no_requests.endpoint();
    for empty_field in [3, 4] {
        let fixture = Fixture::new();
        let before = br#"{"language":"ja","llm_enabled":true,"llm_scope":"local","llm_protocol":"auto","llm_model":"auto"}"#;
        fs::write(fixture.settings(), before).unwrap();
        let mut terminal = Terminal::start(&fixture, true, true);
        let answers = ["cloud", "endpoint", "", endpoint.as_str()];
        fill(
            &mut terminal,
            &fixture,
            Some(before),
            &answers[..empty_field],
        );
        terminal.expect(PROMPTS[empty_field]);
        terminal.line("");
        let (status, output) = terminal.finish();
        assert!(
            !status.success(),
            "cloud accepted missing explicit field: {output}"
        );
        fixture.assert_bytes(before);
    }
    no_requests.assert_idle();
}

#[test]
fn invalid_terminal_fields_are_not_echoed_or_saved() {
    let no_requests = NoRequests::new();
    let endpoint = no_requests.endpoint();
    let oversized = "S".repeat(2049);
    let cases = [
        (0, "SYNTHETIC_BAD_SCOPE"),
        (1, "SYNTHETIC_BAD_ADDRESS_MODE"),
        (2, "SYNTHETIC_BAD_PROTOCOL"),
        (3, "https://127.0.0.1/\u{1b}SYNTHETIC_CONTROL"),
        (4, "synthetic\u{1b}CONTROL_MODEL"),
        (5, "SYNTHETIC_LITERAL_KEY=not-a-credential"),
        (6, "SYNTHETIC_BAD_TIMEOUT"),
        (0, oversized.as_str()),
    ];
    for (invalid_field, invalid) in cases {
        let fixture = Fixture::new();
        let before = baseline(&fixture, &endpoint, "chat-completions");
        let mut terminal = Terminal::start(&fixture, true, true);
        fill(
            &mut terminal,
            &fixture,
            Some(&before),
            &vec![""; invalid_field],
        );
        terminal.expect(PROMPTS[invalid_field]);
        terminal.line(invalid);
        let (status, output) = terminal.finish();
        assert!(!status.success(), "invalid setup field accepted");
        assert!(
            !output.contains(invalid),
            "invalid value leaked in the setup transcript"
        );
        assert!(!output.contains('\u{1b}'));
        fixture.assert_bytes(&before);
    }
    no_requests.assert_idle();
}

#[test]
fn only_explicit_save_can_confirm_the_terminal_preview() {
    let no_requests = NoRequests::new();
    for confirmation in ["", "yes", "SAVE", " save "] {
        let fixture = Fixture::new();
        let before = baseline(&fixture, &no_requests.endpoint(), "chat-completions");
        let mut terminal = Terminal::start(&fixture, true, true);
        fill(&mut terminal, &fixture, Some(&before), &[""; 7]);
        terminal.expect(PROMPTS[7]);
        fixture.assert_bytes(&before);
        terminal.line(confirmation);
        assert_cancelled(terminal);
        fixture.assert_bytes(&before);
    }
    no_requests.assert_idle();
}

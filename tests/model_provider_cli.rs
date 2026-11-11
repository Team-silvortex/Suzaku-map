//! Configuration tests use private files and never contact a real model service.
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-model-cli-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"))
            .args(args)
            .env("SUZAKU_IME_CONFIG", self.0.join("settings.json"))
            .env("XDG_CONFIG_HOME", &self.0)
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // A regression in configuration loading must fail, not hang the test suite.
        // These configuration-only commands produce less than one pipe's capacity.
        let deadline = Instant::now() + Duration::from_secs(2);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!("configuration command blocked: {args:?}; {output:?}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        child.wait_with_output().unwrap()
    }
    fn saved(&self) -> Value {
        serde_json::from_str(&fs::read_to_string(self.0.join("settings.json")).unwrap()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn cloud_configuration_never_grants_consent_implicitly_or_contacts_a_service_for_status() {
    let fixture = Fixture::new();
    success(fixture.run(&[
        "model",
        "configure",
        "--scope",
        "cloud",
        "--endpoint",
        "https://unreachable.invalid/v1/chat/completions",
        "--model",
        "any-family",
        "--api-key-env",
        "SUZAKU_TEST_MODEL_KEY",
    ]));
    let configured = fixture.saved();
    assert_eq!(configured["llm_cloud_consent"], false);
    assert_eq!(configured["llm_enabled"], false);
    assert_eq!(configured["llm_api_key_env"], "SUZAKU_TEST_MODEL_KEY");
    assert!(success(fixture.run(&["model", "status"])).contains("no remote request"));
    let rejected = fixture.run(&["model", "probe", "en"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stdout).contains("云端联想尚未授权"));
    assert_eq!(fixture.saved(), configured);
    success(fixture.run(&["model", "configure", "--cloud-consent", "true"]));
    assert_eq!(fixture.saved()["llm_cloud_consent"], true);
    success(fixture.run(&["model", "configure", "--model", "another-family"]));
    assert_eq!(fixture.saved()["llm_cloud_consent"], false);
    success(fixture.run(&["llama", "configure"]));
    assert_eq!(fixture.saved()["llm_scope"], "local");
    assert_eq!(fixture.saved()["llm_model"], "auto");
    assert_eq!(fixture.saved()["llm_api_key_env"], Value::Null);
}

#[test]
fn invalid_or_literal_credentials_never_replace_existing_settings() {
    let fixture = Fixture::new();
    success(fixture.run(&["llama", "configure", "--model", "my-local-model"]));
    let before = fs::read(fixture.0.join("settings.json")).unwrap();
    for args in [
        vec![
            "model",
            "configure",
            "--endpoint",
            "http://remote.invalid/v1/chat/completions",
        ],
        vec![
            "model",
            "configure",
            "--api-key",
            "synthetic-not-a-real-key",
        ],
        vec!["model", "configure", "--cloud-consent", "yes"],
    ] {
        assert!(!fixture.run(&args).status.success());
        assert_eq!(fs::read(fixture.0.join("settings.json")).unwrap(), before);
    }
}

#[test]
fn concurrent_configuration_writers_fail_without_replacing_settings() {
    let fixture = Fixture::new();
    success(fixture.run(&["model", "configure", "--model", "before"]));
    let path = fixture.0.join("settings.json");
    let before = fs::read(&path).unwrap();
    let _running = suzaku_map::data::files::DataLease::acquire(
        &suzaku_map::data::paths::lock_for_settings(&path).unwrap(),
        false,
    )
    .unwrap();
    let writer = suzaku_map::data::files::DataLease::settings_writer(&path).unwrap();
    let rejected = fixture.run(&["model", "configure", "--model", "not-written"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("正在保存"));
    assert_eq!(fs::read(&path).unwrap(), before);
    drop(writer);
    success(fixture.run(&["model", "configure", "--model", "after"]));
    assert_eq!(fixture.saved()["llm_model"], "after");
}

#[test]
fn settings_read_limits_and_invalid_contents_never_overwrite_the_source() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.json");
    let mut exact = br#"{"language":"ja","llm_enabled":false}"#.to_vec();
    exact.resize(65_536, b' ');
    fs::write(&path, &exact).unwrap();
    success(fixture.run(&["model", "configure", "--model", "synthetic-model"]));
    assert_eq!(fixture.saved()["language"], "ja");
    exact.push(b' ');
    for raw in [exact, vec![0xff], b"{".to_vec(), b"[]".to_vec()] {
        fs::write(&path, &raw).unwrap();
        let result = fixture.run(&["model", "configure", "--model", "not-applied"]);
        assert!(!result.status.success());
        assert_eq!(fs::read(&path).unwrap(), raw);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn special_configuration_files_are_rejected_without_blocking_or_replacement() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.json");
    let rejected = || {
        let result = fixture.run(&["model", "configure", "--model", "not-applied"]);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("无法读取输入法设置"));
    };

    fs::create_dir(&path).unwrap();
    rejected();
    assert!(path.is_dir());
    fs::remove_dir(&path).unwrap();

    let socket = UnixListener::bind(&path).unwrap();
    rejected();
    drop(socket);
    fs::remove_file(&path).unwrap();

    let fifo = fixture.0.join("pipe");
    let c_path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    for linked in [false, true] {
        if linked {
            symlink(&fifo, &path).unwrap();
        } else {
            fs::rename(&fifo, &path).unwrap();
        }
        let before = fs::symlink_metadata(&path).unwrap().file_type();
        rejected();
        assert_eq!(fs::symlink_metadata(&path).unwrap().file_type(), before);
        if linked {
            fs::remove_file(&path).unwrap();
        } else {
            fs::rename(&path, &fifo).unwrap();
        }
    }
}

#[cfg(unix)]
#[test]
fn symlinks_to_regular_configuration_files_remain_readable() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let target = fixture.0.join("target.json");
    let raw = br#"{"language":"ja","llm_scope":"cloud","llm_model":"synthetic-model","llm_endpoint":"https://unreachable.invalid/v1/chat/completions"}"#;
    fs::write(&target, raw).unwrap();
    let path = fixture.0.join("settings.json");
    symlink(&target, &path).unwrap();
    let output = success(fixture.run(&["model", "status"]));
    assert!(output.contains("input-language: ja"));
    assert!(output.contains("no remote request"));
    assert!(path.is_symlink());
    assert_eq!(fs::read(&target).unwrap(), raw);
}

#[test]
fn model_help_does_not_read_or_replace_a_broken_configuration() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.json");
    let raw = b"{this deliberately is not valid model configuration";
    fs::write(&path, raw).unwrap();
    for command in ["help", "--help"] {
        let output = success(fixture.run(&["model", command]));
        for entry in ["configure", "--base-url", "check", "probe"] {
            assert!(
                output.contains(entry),
                "missing help entry {entry}: {output}"
            );
        }
        assert_eq!(fs::read(&path).unwrap(), raw);
    }
}

#[test]
fn base_url_configuration_expands_only_the_generation_route_and_preserves_opt_in() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.json");
    fs::write(&path, br#"{"language":"ja","llm_enabled":true}"#).unwrap();
    success(fixture.run(&[
        "model",
        "configure",
        "--base-url",
        "https://models.example/v1",
        "--scope",
        "cloud",
        "--model",
        "demo",
        "--api-key-env",
        "SUZAKU_TEST_MODEL_KEY",
    ]));
    let saved = fixture.saved();
    assert_eq!(
        saved["llm_endpoint"],
        "https://models.example/v1/chat/completions"
    );
    assert_eq!(saved["llm_protocol"], "openai-compatible");
    assert_eq!(saved["llm_scope"], "cloud");
    assert_eq!(saved["llm_model"], "demo");
    assert_eq!(saved["llm_api_key_env"], "SUZAKU_TEST_MODEL_KEY");
    assert_eq!(saved["llm_cloud_consent"], false);
    assert_eq!(saved["llm_enabled"], true);
    assert_eq!(saved["language"], "ja");

    // A complete generation endpoint must never gain an extra route segment.
    success(fixture.run(&[
        "model",
        "configure",
        "--endpoint",
        "https://models.example/custom/generate",
        "--protocol",
        "openai-compatible",
    ]));
    assert_eq!(
        fixture.saved()["llm_endpoint"],
        "https://models.example/custom/generate"
    );
    assert_eq!(fixture.saved()["llm_cloud_consent"], false);
}

#[test]
fn base_url_conflicts_are_rejected_in_both_argument_orders_without_saving() {
    let fixture = Fixture::new();
    success(fixture.run(&["model", "configure", "--model", "before"]));
    let path = fixture.0.join("settings.json");
    let before = fs::read(&path).unwrap();
    for conflicting in [
        vec![
            "--base-url",
            "https://models.example/v1",
            "--endpoint",
            "https://models.example/custom/generate",
        ],
        vec![
            "--endpoint",
            "https://models.example/custom/generate",
            "--base-url",
            "https://models.example/v1",
        ],
        vec![
            "--base-url",
            "https://models.example/v1",
            "--protocol",
            "ollama",
        ],
        vec![
            "--protocol",
            "ollama",
            "--base-url",
            "https://models.example/v1",
        ],
        vec![
            "--base-url",
            "https://models.example/v1",
            "--protocol",
            "auto",
        ],
        vec![
            "--protocol",
            "auto",
            "--base-url",
            "https://models.example/v1",
        ],
    ] {
        let mut args = vec!["model", "configure", "--scope", "cloud", "--model", "demo"];
        args.extend(conflicting);
        let rejected = fixture.run(&args);
        assert!(
            !rejected.status.success(),
            "conflicting configuration accepted: {args:?}"
        );
        let diagnostic = String::from_utf8_lossy(&rejected.stderr);
        assert!(
            diagnostic.contains("base-url"),
            "missing conflict diagnostic: {diagnostic}"
        );
        assert!(
            diagnostic.contains("endpoint") || diagnostic.contains("protocol"),
            "missing conflicting parameter: {diagnostic}"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn unknown_and_inline_credential_options_never_echo_values_or_change_configuration() {
    let fixture = Fixture::new();
    success(fixture.run(&["model", "configure", "--model", "before"]));
    let path = fixture.0.join("settings.json");
    let before = fs::read(&path).unwrap();
    let marker = "SYNTHETIC_SECRET_MODEL_CLI_NO_ECHO_42";
    let inline_key = format!("--api-key={marker}");
    let inline_unknown = format!("--unknown-model-option={marker}");
    for args in [
        vec!["model", "configure", "--api-key", marker],
        vec!["model", "configure", inline_key.as_str(), "unused"],
        vec!["model", "configure", inline_unknown.as_str(), "unused"],
        vec![
            "model",
            "probe",
            "en",
            "--once",
            inline_key.as_str(),
            "unused",
        ],
    ] {
        let rejected = fixture.run(&args);
        assert!(!rejected.status.success());
        assert!(!String::from_utf8_lossy(&rejected.stdout).contains(marker));
        assert!(!String::from_utf8_lossy(&rejected.stderr).contains(marker));
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

/// A private loopback fixture records every connection, not only valid HTTP.
/// This makes accidental metadata checks or TLS attempts observable as well.
struct ModelCliServer {
    address: std::net::SocketAddr,
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    connections: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl ModelCliServer {
    fn new() -> Self {
        use std::io::{Read, Write};
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let connections = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (stop, count, received) = (stopped.clone(), connections.clone(), requests.clone());
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(4);
            while !stop.load(Ordering::SeqCst) && Instant::now() < deadline {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("private model listener: {error}"),
                };
                count.fetch_add(1, Ordering::SeqCst);
                stream
                    .set_read_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut block = [0_u8; 4096];
                loop {
                    match stream.read(&mut block) {
                        Ok(0) => break,
                        Ok(size) => bytes.extend_from_slice(&block[..size]),
                        Err(_) => break,
                    }
                    assert!(bytes.len() <= 131_072, "oversized private model request");
                    if let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n")
                    {
                        let headers = String::from_utf8_lossy(&bytes[..header_end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= header_end + 4 + length {
                            break;
                        }
                    }
                }
                received
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&bytes).into_owned());
                let body = serde_json::json!({"choices":[{"finish_reason":"stop",
                    "message":{"content":"{\"candidates\":[\"hello world\",\"hello again\"]}"}}]})
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Self {
            address,
            stopped,
            connections,
            requests,
            worker: Some(worker),
        }
    }

    fn endpoint(&self, scheme: &str) -> String {
        format!("{scheme}://{}/v1/chat/completions", self.address)
    }

    fn finish(mut self) -> (usize, Vec<String>) {
        self.stop();
        let count = self.connections.load(std::sync::atomic::Ordering::SeqCst);
        let requests = self.requests.lock().unwrap().clone();
        (count, requests)
    }

    fn stop(&mut self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

impl Drop for ModelCliServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[test]
fn explicit_modern_chat_profile_uses_bounded_nonreasoning_wire_without_model_name_detection() {
    let fixture = Fixture::new();
    let server = ModelCliServer::new();
    let endpoint = server.endpoint("http");
    let output = success(fixture.run(&[
        "model",
        "probe",
        "en",
        "--once",
        "--scope",
        "local",
        "--protocol",
        "chat-completions",
        "--endpoint",
        &endpoint,
        "--model",
        "arbitrary-family-not-a-gpt-model",
        "--timeout-ms",
        "1200",
    ]));
    assert!(output.contains("hello world"));
    assert!(!fixture.0.join("settings.json").exists());
    let (count, requests) = server.finish();
    assert_eq!(count, 1);
    let (_, body) = requests[0].split_once("\r\n\r\n").unwrap();
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "arbitrary-family-not-a-gpt-model");
    assert_eq!(body["reasoning_effort"], "none");
    assert_eq!(body["max_completion_tokens"], 256);
    assert!(body.get("max_tokens").is_none());
    assert_eq!(body["stream"], false);
}

#[test]
fn model_check_is_offline_and_leaves_the_exact_configuration_untouched() {
    let fixture = Fixture::new();
    let server = ModelCliServer::new();
    let endpoint = server.endpoint("http");
    success(fixture.run(&[
        "model",
        "configure",
        "--model",
        "synthetic-cli-model",
        "--endpoint",
        &endpoint,
        "--protocol",
        "openai-compatible",
    ]));
    let path = fixture.0.join("settings.json");
    let before = fs::read(&path).unwrap();
    let output = success(fixture.run(&["model", "check"])).to_ascii_lowercase();
    assert!(
        output.contains("no network"),
        "offline preflight needs a clear boundary: {output}"
    );
    assert!(output.contains("untouched") || output.contains("unchanged"));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(
        server.finish().0,
        0,
        "offline check connected to a model service"
    );
}

#[test]
fn model_check_rejects_invalid_configuration_without_replacing_it() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.json");
    for raw in [b"{".as_slice(), b"[]", br#"{"llm_timeout_ms":5001}"#] {
        fs::write(&path, raw).unwrap();
        let rejected = fixture.run(&["model", "check"]);
        assert!(!rejected.status.success());
        assert_eq!(fs::read(&path).unwrap(), raw);
    }
}

#[test]
fn probe_once_uses_one_temporary_compatible_request_without_saving_or_discovery() {
    let fixture = Fixture::new();
    success(fixture.run(&["model", "configure", "--model", "persisted-baseline"]));
    let path = fixture.0.join("settings.json");
    let before = fs::read(&path).unwrap();
    let server = ModelCliServer::new();
    let endpoint = server.endpoint("http");
    let output = success(fixture.run(&[
        "model",
        "probe",
        "en",
        "--once",
        "--scope",
        "local",
        "--protocol",
        "openai-compatible",
        "--endpoint",
        &endpoint,
        "--model",
        "synthetic-cli-model",
        "--timeout-ms",
        "1200",
    ]));
    assert!(output.contains("hello world"));
    assert_eq!(fs::read(&path).unwrap(), before);
    let (count, requests) = server.finish();
    assert_eq!(
        count, 1,
        "probe --once must not discover, warm up, or send every language sample"
    );
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
    assert!(!requests[0].to_ascii_lowercase().contains("authorization:"));
    let (_, body) = requests[0].split_once("\r\n\r\n").unwrap();
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "synthetic-cli-model");
    assert_eq!(body["stream"], false);
    let input: Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(input["raw_composition"], "hello");
}

#[test]
fn temporary_cloud_probe_rejects_missing_consent_or_credentials_before_any_connection() {
    let fixture = Fixture::new();
    success(fixture.run(&["model", "configure", "--model", "persisted-baseline"]));
    let path = fixture.0.join("settings.json");
    let before = fs::read(&path).unwrap();
    let missing_key = format!(
        "SUZAKU_TEST_MODEL_MISSING_KEY_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    for authorized in [false, true] {
        let server = ModelCliServer::new();
        let endpoint = server.endpoint("https");
        let mut args = vec![
            "model",
            "probe",
            "en",
            "--once",
            "--scope",
            "cloud",
            "--protocol",
            "openai-compatible",
            "--endpoint",
            &endpoint,
            "--model",
            "synthetic-cli-model",
            "--api-key-env",
            &missing_key,
        ];
        if authorized {
            args.extend(["--cloud-consent", "true"]);
        }
        let rejected = fixture.run(&args);
        assert!(!rejected.status.success());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(
            server.finish().0,
            0,
            "unauthorized/uncredentialed probe connected"
        );
    }
}

#[test]
fn configure_dry_run_never_creates_or_replaces_settings_or_changes_llm_opt_in() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.json");
    let server = ModelCliServer::new();
    let base_url = format!("https://{}/v1", server.address);
    let args = [
        "model",
        "configure",
        "--dry-run",
        "--scope",
        "cloud",
        "--base-url",
        base_url.as_str(),
        "--model",
        "prospective-model",
        "--api-key-env",
        "SUZAKU_TEST_MODEL_KEY",
    ];
    let missing_output = success(fixture.run(&args));
    assert!(missing_output.contains("LLM enabled: false"));
    assert!(!path.exists(), "dry-run created a missing settings file");

    let authorized = serde_json::json!({
        "language": "ja", "llm_enabled": true, "llm_scope": "cloud",
        "llm_protocol": "openai-compatible", "llm_cloud_consent": true,
        "llm_endpoint": "https://old.invalid/v1/chat/completions",
        "llm_model": "authorized-old-model", "llm_api_key_env": "SUZAKU_TEST_MODEL_KEY"
    });
    let before = serde_json::to_vec(&authorized).unwrap();
    fs::write(&path, &before).unwrap();
    let existing_output = success(fixture.run(&args));
    assert!(existing_output.contains("LLM enabled: true"));
    assert!(existing_output.contains("cloud consent: false"));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fixture.saved(), authorized);
    assert_eq!(server.finish().0, 0, "dry-run contacted a model service");
}

#[test]
fn model_check_previews_temporary_base_url_overrides_without_network_or_saving() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.json");
    let before = br#"{"language":"ja","llm_enabled":true,"llm_model":"persisted-baseline"}"#;
    fs::write(&path, before).unwrap();
    let server = ModelCliServer::new();
    let base_url = format!("https://{}/v1", server.address);
    let full_endpoint = server.endpoint("https");
    let output = success(fixture.run(&[
        "model",
        "check",
        "--base-url",
        &base_url,
        "--model",
        "preview-compatible-model",
        "--scope",
        "cloud",
        "--api-key-env",
        "SUZAKU_TEST_MODEL_KEY",
    ]));
    assert!(output.contains(&full_endpoint));
    assert!(output.contains("preview-compatible-model"));
    assert!(output.contains("SUZAKU_TEST_MODEL_KEY"));
    assert!(output.contains("openai-compatible"));
    assert!(output.contains("LLM enabled: true"));
    assert!(output.to_ascii_lowercase().contains("no network"));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(
        server.finish().0,
        0,
        "temporary check contacted a model service"
    );
}

#[test]
fn invalid_single_probe_requests_never_connect_or_rewrite_configuration() {
    let fixture = Fixture::new();
    let server = ModelCliServer::new();
    let endpoint = server.endpoint("http");
    success(fixture.run(&[
        "model",
        "configure",
        "--model",
        "persisted-baseline",
        "--endpoint",
        &endpoint,
        "--protocol",
        "openai-compatible",
    ]));
    let path = fixture.0.join("settings.json");
    let before = fs::read(&path).unwrap();
    for tail in [
        vec!["all", "--once"],
        vec!["unsupported-language", "--once"],
        vec!["en", "--once", "--model"],
        vec!["en", "--once", "--timeout-ms", "not-an-integer"],
        vec!["en", "--once", "--once"],
        vec!["en", "--once", "--unknown-option", "unused"],
    ] {
        let mut args = vec!["model", "probe"];
        args.extend(tail);
        let rejected = fixture.run(&args);
        assert!(
            !rejected.status.success(),
            "invalid probe accepted: {args:?}"
        );
        assert!(!rejected.stderr.is_empty());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    assert_eq!(
        server.finish().0,
        0,
        "invalid probe contacted a model service"
    );
}

/// No inherited credentials or desktop environment; values below are synthetic.
/// Retain the original configuration-command two-second deadline unchanged.
fn private_model_command(
    fixture: &Fixture,
    args: &[&str],
    overrides: &[(&str, Option<std::ffi::OsString>)],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"));
    command
        .args(args)
        .env_clear()
        .current_dir(&fixture.0)
        .env("HOME", fixture.0.join("home"))
        .env("XDG_CONFIG_HOME", fixture.0.join("config"))
        .env("XDG_DATA_HOME", fixture.0.join("data"))
        .env("XDG_STATE_HOME", fixture.0.join("state"))
        .env("XDG_CACHE_HOME", fixture.0.join("cache"))
        .env("LC_ALL", "C")
        .env("SUZAKU_IME_CONFIG", fixture.0.join("settings.json"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in overrides {
        if let Some(value) = value {
            command.env(name, value);
        } else {
            command.env_remove(name);
        }
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!("private configuration command blocked: {args:?}; {output:?}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn model_paths_and_guide_do_not_read_broken_settings_or_create_directories() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.json");
    let raw = b"{broken model configuration";
    fs::write(&path, raw).unwrap();
    let output = success(private_model_command(&fixture, &["model", "paths"], &[]));
    assert!(output.contains(&path.to_string_lossy().escape_debug().to_string()));
    assert!(output.contains("SUZAKU_IME_CONFIG"));
    assert!(output.contains("no configuration files read or created"));
    assert!(output.contains("host") && output.contains("not inspected"));
    let guide = success(private_model_command(&fixture, &["model", "guide"], &[]));
    for command in [
        "model paths",
        "model check",
        "--credentials",
        "model probe en --once",
    ] {
        assert!(guide.contains(command), "missing setup guidance: {command}");
    }
    assert_eq!(fs::read(&path).unwrap(), raw);
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[test]
fn relative_and_empty_model_path_overrides_preserve_actual_settings_semantics() {
    let fixture = Fixture::new();
    let relative = "relative-settings.json";
    let path = fixture.0.join(relative);
    let raw = b"not JSON, paths must not parse it";
    fs::write(&path, raw).unwrap();
    let output = success(private_model_command(
        &fixture,
        &["model", "paths"],
        &[("SUZAKU_IME_CONFIG", Some(relative.into()))],
    ));
    assert!(output.contains(&path.to_string_lossy().escape_debug().to_string()));
    assert!(output.contains("SUZAKU_IME_CONFIG"));
    let empty = private_model_command(
        &fixture,
        &["model", "paths"],
        &[("SUZAKU_IME_CONFIG", Some("".into()))],
    );
    assert!(!empty.status.success());
    assert!(String::from_utf8_lossy(&empty.stderr).contains("SUZAKU_IME_CONFIG"));
    assert_eq!(fs::read(&path).unwrap(), raw);
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[test]
fn model_paths_and_guide_escape_control_characters_without_touching_the_path() {
    let fixture = Fixture::new();
    let synthetic_path = fixture.0.join("synthetic\n\u{1b}[31m-settings.json");
    let escaped = synthetic_path.to_string_lossy().escape_debug().to_string();
    for command in ["paths", "guide"] {
        let output = success(private_model_command(
            &fixture,
            &["model", command],
            &[("SUZAKU_IME_CONFIG", Some(synthetic_path.clone().into()))],
        ));
        assert!(output.contains(&escaped));
        assert!(output.contains("\\n") && output.contains("\\u{1b}"));
        assert!(!output.contains('\u{1b}'));
        assert!(!output.contains("synthetic\n"));
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn model_paths_report_xdg_and_home_fallback_without_creating_either() {
    let fixture = Fixture::new();
    let absolute_xdg = fixture.0.join("config/suzaku-ime/settings.json");
    let output = success(private_model_command(
        &fixture,
        &["model", "paths"],
        &[("SUZAKU_IME_CONFIG", None)],
    ));
    assert!(output.contains(&absolute_xdg.to_string_lossy().escape_debug().to_string()));
    assert!(output.contains("source: XDG_CONFIG_HOME"));
    for xdg in [None, Some("".into()), Some("relative-config".into())] {
        let output = success(private_model_command(
            &fixture,
            &["model", "paths"],
            &[("SUZAKU_IME_CONFIG", None), ("XDG_CONFIG_HOME", xdg)],
        ));
        assert!(
            output.contains(
                &fixture
                    .0
                    .join("home/.config/suzaku-ime/settings.json")
                    .to_string_lossy()
                    .escape_debug()
                    .to_string()
            )
        );
        assert!(output.contains("source: HOME/.config"));
    }
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
}

fn synthetic_cloud_precheck_settings(fixture: &Fixture) -> Vec<u8> {
    let bytes = br#"{"llm_scope":"cloud","llm_protocol":"openai-compatible","llm_endpoint":"https://unreachable.invalid/v1/chat/completions","llm_model":"synthetic-model","llm_api_key_env":"SUZAKU_SYNTHETIC_CREDENTIAL","llm_cloud_consent":false,"llm_enabled":false}"#.to_vec();
    fs::write(fixture.0.join("settings.json"), &bytes).unwrap();
    bytes
}

#[test]
fn credential_precheck_is_explicit_process_only_and_never_grants_consent() {
    let fixture = Fixture::new();
    let before = synthetic_cloud_precheck_settings(&fixture);
    let marker = "SYNTHETIC_VALID_CREDENTIAL_PRECHECK_42";
    let valid = success(private_model_command(
        &fixture,
        &["model", "check", "--credentials"],
        &[("SUZAKU_SYNTHETIC_CREDENTIAL", Some(marker.into()))],
    ));
    assert!(valid.contains("current-process credential status: present"));
    assert!(valid.contains("cloud consent: false") && valid.contains("LLM enabled: false"));
    assert!(
        valid.contains("No authentication request") && valid.contains("host-process inspection")
    );
    assert!(!valid.contains(marker));
    assert_eq!(fs::read(fixture.0.join("settings.json")).unwrap(), before);

    // No flag means no credential read or format check, even when a value is invalid.
    let uninspected = success(private_model_command(
        &fixture,
        &["model", "check"],
        &[(
            "SUZAKU_SYNTHETIC_CREDENTIAL",
            Some("invalid value marker".into()),
        )],
    ));
    assert!(uninspected.contains("Credential values were not read"));
    assert!(!uninspected.contains("current-process credential status:"));
    assert!(!uninspected.contains("invalid value marker"));
    assert_eq!(fs::read(fixture.0.join("settings.json")).unwrap(), before);
}

#[test]
fn missing_or_invalid_explicit_credentials_fail_without_values_or_settings_writes() {
    let fixture = Fixture::new();
    let before = synthetic_cloud_precheck_settings(&fixture);
    for value in [
        None,
        Some(String::new()),
        Some("SYNTHETIC INVALID VALUE".into()),
        Some("SYNTHETIC\nCONTROL_VALUE".into()),
        Some("合成INVALID_VALUE".into()),
        Some("SYNTHETIC_OVERSIZED_VALUE_".repeat(400)),
    ] {
        let expected = if value.is_none() {
            "missing"
        } else {
            "invalid"
        };
        let output = private_model_command(
            &fixture,
            &["model", "check", "--credentials"],
            &[("SUZAKU_SYNTHETIC_CREDENTIAL", value.clone().map(Into::into))],
        );
        assert!(!output.status.success());
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(text.contains(&format!("current-process credential status: {expected}")));
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            assert!(!text.contains(&value), "credential value leaked");
        }
        assert_eq!(fs::read(fixture.0.join("settings.json")).unwrap(), before);
    }
}

#[test]
fn explicit_credential_precheck_accepts_local_and_cloud_without_authentication() {
    let fixture = Fixture::new();
    let output = success(private_model_command(
        &fixture,
        &["model", "check", "--credentials"],
        &[],
    ));
    assert!(output.contains("current-process credential status: not-required"));
    assert!(!fixture.0.join("settings.json").exists());
    let raw = br#"{"llm_scope":"cloud","llm_model":"no-auth-model","llm_endpoint":"https://unreachable.invalid/v1/chat/completions","llm_api_key_env":null}"#;
    fs::write(fixture.0.join("settings.json"), raw).unwrap();
    let output = success(private_model_command(
        &fixture,
        &["model", "check", "--credentials"],
        &[],
    ));
    assert!(output.contains("current-process credential status: not-required"));
    assert_eq!(fs::read(fixture.0.join("settings.json")).unwrap(), raw);
}

#[test]
fn path_guide_and_credential_argument_errors_never_echo_unknown_values() {
    let fixture = Fixture::new();
    let before = synthetic_cloud_precheck_settings(&fixture);
    let marker = "SYNTHETIC_ARGUMENT_NO_ECHO_42";
    for args in [
        vec!["model", "paths", marker],
        vec!["model", "guide", marker],
        vec!["model", "check", "--credentials", "--credentials"],
        vec!["model", "check", "--credentials", "--unknown", marker],
    ] {
        let output = private_model_command(&fixture, &args, &[]);
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains(marker));
        assert!(!String::from_utf8_lossy(&output.stderr).contains(marker));
        assert_eq!(fs::read(fixture.0.join("settings.json")).unwrap(), before);
    }
}

#[cfg(unix)]
#[test]
fn model_status_escapes_a_real_configuration_path_without_changing_its_bytes() {
    let fixture = Fixture::new();
    let path = fixture.0.join("synthetic\n\u{1b}[31m-settings.json");
    let raw = br#"{"llm_scope":"cloud","llm_protocol":"openai-compatible","llm_endpoint":"https://unreachable.invalid/v1/chat/completions","llm_model":"synthetic-model","llm_cloud_consent":false}"#;
    fs::write(&path, raw).unwrap();
    let output = success(private_model_command(
        &fixture,
        &["model", "status"],
        &[("SUZAKU_IME_CONFIG", Some(path.clone().into()))],
    ));
    assert!(output.contains(&path.to_string_lossy().escape_debug().to_string()));
    assert!(output.contains("Cloud configuration only; no remote request was made."));
    assert!(!output.contains('\u{1b}'));
    assert!(!output.contains("synthetic\n"));
    assert_eq!(fs::read(&path).unwrap(), raw);
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[test]
fn explicit_low_reasoning_probe_omits_sampling_and_does_not_change_the_old_profile() {
    let fixture = Fixture::new();
    let server = ModelCliServer::new();
    let endpoint = server.endpoint("http");
    let output = success(fixture.run(&[
        "model",
        "probe",
        "en",
        "--once",
        "--scope",
        "local",
        "--protocol",
        "chat-completions-low",
        "--endpoint",
        &endpoint,
        "--model",
        "arbitrary-family-not-an-official-model",
        "--timeout-ms",
        "1200",
    ]));
    assert!(output.contains("hello world"));
    assert!(!fixture.0.join("settings.json").exists());
    let (count, requests) = server.finish();
    assert_eq!(count, 1);
    let (_, body) = requests[0].split_once("\r\n\r\n").unwrap();
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["reasoning_effort"], "low");
    assert_eq!(body["max_completion_tokens"], 256);
    assert_eq!(body["stream"], false);
    for absent in ["max_tokens", "temperature", "top_p", "top_k", "logprobs"] {
        assert!(body.get(absent).is_none(), "unexpected {absent}");
    }
}

#[test]
fn low_reasoning_base_configuration_revokes_old_profile_consent_and_preserves_the_target() {
    let fixture = Fixture::new();
    success(fixture.run(&[
        "model",
        "configure",
        "--scope",
        "cloud",
        "--protocol",
        "chat-completions",
        "--base-url",
        "https://unreachable.invalid/gateway/v1",
        "--model",
        "arbitrary-family",
    ]));
    success(fixture.run(&["model", "configure", "--cloud-consent", "true"]));
    let previous = fixture.saved();
    success(fixture.run(&[
        "model",
        "configure",
        "--protocol",
        "chat-completions-low",
        "--base-url",
        "https://unreachable.invalid/gateway/v1",
    ]));
    let mut expected = previous;
    expected["llm_protocol"] = "chat-completions-low".into();
    expected["llm_cloud_consent"] = false.into();
    assert_eq!(fixture.saved(), expected);
    assert!(success(fixture.run(&["model", "check"])).contains("protocol: chat-completions-low"));
    assert_eq!(fixture.saved(), expected);
}

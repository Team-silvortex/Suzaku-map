//! Deterministic event ordering for the real native layout fixture functions.
//! No display, input injection or personal IBus session is used by this guard.
#![cfg(target_os = "linux")]

#[test]
fn settings_focus_requires_fresh_handoffs_and_preserves_failure_checks() {
    let output = std::process::Command::new("/usr/bin/python3")
        .args(["-B", "scripts/test-native-settings-focus.py"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run the native settings focus regression guard");
    assert!(
        output.status.success(),
        "native settings focus guard failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

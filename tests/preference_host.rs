//! Default-policy acceptance on a task-owned, display-less D-Bus/IBus session.
//! Linux all-feature CI already supplies GI/IBus/cc; missing dependencies fail,
//! never silently skip or touch the user's input service/files.
#[cfg(all(target_os = "linux", feature = "linux-ibus"))]
#[test]
fn native_preferences_restore_clear_and_slow_io_use_the_real_default_policy() {
    let output = std::process::Command::new("/usr/bin/python3")
        .args([
            "-B",
            "scripts/test-preference-host.py",
            "--host",
            env!("CARGO_BIN_EXE_linux_ime_host"),
            "--tool",
            env!("CARGO_BIN_EXE_suzaku_tool"),
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "owned native preference gate failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.starts_with("PASS:"))
            .count(),
        9
    );
}

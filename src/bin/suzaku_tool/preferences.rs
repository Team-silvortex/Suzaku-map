//! Read status or explicitly clear through the running host, never open a
//! personal cache independently or bypass its writer/acknowledgement protocol.
pub fn run(args: &[String]) -> i32 {
    let action = match args {
        [] => "status",
        [action] if matches!(action.as_str(), "status" | "clear") => action.as_str(),
        [action] if matches!(action.as_str(), "help" | "--help" | "-h") => {
            println!("Usage: suzaku_tool preferences [status|clear]");
            println!(
                "Status is read-only. Clear requires confirmation from the running Linux host."
            );
            return 0;
        }
        _ => {
            eprintln!("preferences accepts only status or clear, without extra arguments");
            return 1;
        }
    };
    #[cfg(not(target_os = "linux"))]
    {
        let _ = action;
        eprintln!("Native preference management is currently supported on Linux only");
        1
    }
    #[cfg(target_os = "linux")]
    {
        use suzaku_map::platform::linux_ime_control;
        let result = match action {
            "clear" => linux_ime_control::clear_preferences(),
            _ => linux_ime_control::status(),
        };
        match result {
            Ok(status) => {
                let preferences = status.preferences;
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true, "action": action, "enabled": preferences.enabled,
                        "memory_only": preferences.memory_only, "pending": preferences.pending,
                        "entries": preferences.entries, "error": preferences.error
                    })
                );
                0
            }
            Err(error) => {
                eprintln!("{error}");
                1
            }
        }
    }
}

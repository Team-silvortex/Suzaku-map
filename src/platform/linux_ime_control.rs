//! User-only native host settings channel. No input text is involved in these requests.
use super::linux_ipc::Deadline;
use crate::{ime::settings::ImeSettings, languages::BuiltinLanguage};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeImeStatus {
    pub settings: ImeSettings,
    pub prediction: String,
    pub prediction_error: Option<String>,
    pub preferences: NativePreferenceStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePreferenceStatus {
    pub enabled: bool,
    pub memory_only: bool,
    pub pending: bool,
    pub error: Option<String>,
    pub entries: usize,
    pub clear: Option<NativePreferenceClearStatus>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreferenceClearReceipt {
    pub epoch: String,
    pub id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativePreferenceClearState {
    Pending,
    Complete,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePreferenceClearStatus {
    pub receipt: PreferenceClearReceipt,
    pub state: NativePreferenceClearState,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreferenceClearFailure {
    pub message: String,
    pub receipt: Option<PreferenceClearReceipt>,
}

pub fn status() -> Result<NativeImeStatus, String> {
    request("S")
}
pub fn set_language(language: BuiltinLanguage) -> Result<NativeImeStatus, String> {
    request(&format!("L{}", language.id()))
}
pub fn set_llm_enabled(enabled: bool) -> Result<NativeImeStatus, String> {
    request(if enabled { "P1" } else { "P0" })
}
pub fn set_panel_ime_settings(
    patch: crate::ime::settings::PanelImeSettingsPatch,
) -> Result<NativeImeStatus, String> {
    request(&panel_settings_command(patch)?)
}

fn panel_settings_command(
    patch: crate::ime::settings::PanelImeSettingsPatch,
) -> Result<String, String> {
    let raw = patch.to_json().to_string();
    crate::ime::settings::PanelImeSettingsPatch::from_json(&raw)?;
    // Keep the existing on/off control usable while an older host is still running.
    if let (Some(enabled), None, None, None) = (
        patch.enabled,
        patch.temperature_tenths,
        patch.shortcut_profile,
        patch.shortcut_bindings,
    ) {
        return Ok(if enabled { "P1" } else { "P0" }.into());
    }
    Ok(format!("U{raw}"))
}
pub fn reload_settings() -> Result<NativeImeStatus, String> {
    request("R")
}
pub fn clear_preferences() -> Result<NativeImeStatus, String> {
    clear_preferences_at(&socket_path()?, Duration::from_millis(700))
}

pub fn clear_preferences_tracked() -> Result<NativeImeStatus, PreferenceClearFailure> {
    let path = socket_path().map_err(|message| PreferenceClearFailure {
        message,
        receipt: None,
    })?;
    clear_preferences_tracked_at(&path, Duration::from_millis(700))
}

fn socket_path() -> Result<std::path::PathBuf, String> {
    std::env::var_os("SUZAKU_LINUX_IME_SOCKET")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(|root| std::path::PathBuf::from(root).join("suzaku-ime/host.sock"))
        })
        .ok_or_else(|| "无法定位本机输入法服务".into())
}

fn request(command: &str) -> Result<NativeImeStatus, String> {
    request_at(&socket_path()?, command, Duration::from_millis(700))
}

fn request_at(
    path: &std::path::Path,
    command: &str,
    timeout: Duration,
) -> Result<NativeImeStatus, String> {
    let deadline = Deadline::new(timeout);
    parse_status(&exchange(path, command, &deadline)?)
}

fn exchange(
    path: &std::path::Path,
    command: &str,
    deadline: &Deadline<'_>,
) -> Result<String, String> {
    let exchange = || -> std::io::Result<Vec<u8>> {
        let mut stream = deadline.connect(path)?;
        deadline.send(&mut stream, command.as_bytes())?;
        deadline.read_to_end(&mut stream, 8192)
    };
    let bytes = exchange().map_err(|error| match error.kind() {
        std::io::ErrorKind::TimedOut => "输入法设置请求超时，请重试",
        std::io::ErrorKind::InvalidData => "输入法服务返回数据过大",
        _ => "Suzaku 输入法服务未运行或连接已断开",
    })?;
    let response = String::from_utf8(bytes).map_err(|_| "输入法服务返回了无效文本")?;
    Ok(response)
}

fn clear_preferences_at(
    path: &std::path::Path,
    timeout: Duration,
) -> Result<NativeImeStatus, String> {
    clear_preferences_tracked_at(path, timeout).map_err(|failure| failure.message)
}

fn clear_preferences_tracked_at(
    path: &std::path::Path,
    timeout: Duration,
) -> Result<NativeImeStatus, PreferenceClearFailure> {
    let unaccepted = |message| PreferenceClearFailure {
        message,
        receipt: None,
    };
    let deadline = Deadline::new(timeout);
    // F is sent exactly once. Every subsequent request is read-only S, sharing
    // the same original deadline and target. Disconnect/timeout is unconfirmed.
    let raw = exchange(path, "F", &deadline).map_err(unaccepted)?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|_| unaccepted("偏好清除请求未收到有效确认".into()))?;
    if value["ok"] == true {
        let status = parse_status(&raw).map_err(unaccepted)?;
        if status.preferences.enabled {
            return Err(unaccepted("偏好文件清除缺少操作确认，请刷新状态".into()));
        }
        return Ok(status); // Legacy/explicit RAM-only host.
    }
    let id = value["preference_clear_accepted"]
        .as_u64()
        .filter(|id| *id != 0);
    let epoch = value["preferences"]["epoch"]
        .as_str()
        .filter(|epoch| epoch.len() == 32 && epoch.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let (Some(id), Some(epoch)) = (id, epoch) else {
        return parse_status(&raw).map_err(unaccepted);
    };
    if value["preferences"]["enabled"] != true {
        return Err(unaccepted("偏好清除未收到持久化操作确认".into()));
    }
    // Preserve only an acknowledged operation's identity. A later read-only
    // refresh may reconcile this exact clear after the original budget expires.
    let receipt = PreferenceClearReceipt {
        epoch: epoch.into(),
        id,
    };
    let unconfirmed = |message| PreferenceClearFailure {
        message,
        receipt: Some(receipt.clone()),
    };
    loop {
        let raw = exchange(path, "S", &deadline)
            .map_err(|error| unconfirmed(format!("偏好文件清除未获确认，请刷新状态：{error}")))?;
        let next: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|_| unconfirmed("偏好文件清除状态无效，未确认成功".into()))?;
        let preferences = &next["preferences"];
        if preferences["epoch"] != epoch || preferences["clear"]["id"] != id {
            return Err(unconfirmed(
                "输入法宿主或清除任务已更换，未确认原操作成功".into(),
            ));
        }
        let clear = parse_preference_clear(preferences)
            .ok_or_else(|| unconfirmed("偏好文件清除状态不完整，未确认成功".into()))?;
        match clear.state {
            NativePreferenceClearState::Complete => return parse_status(&raw).map_err(unconfirmed),
            NativePreferenceClearState::Failed => {
                return Err(unconfirmed(
                    clear
                        .error
                        .unwrap_or_else(|| "偏好文件清除失败，未确认成功".into()),
                ));
            }
            NativePreferenceClearState::Pending => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

fn parse_preference_clear(preferences: &serde_json::Value) -> Option<NativePreferenceClearStatus> {
    if preferences["enabled"] != true {
        return None;
    }
    let epoch = preferences["epoch"]
        .as_str()
        .filter(|epoch| epoch.len() == 32 && epoch.bytes().all(|byte| byte.is_ascii_hexdigit()))?;
    let clear = &preferences["clear"];
    let id = clear["id"].as_u64().filter(|id| *id != 0)?;
    let state = match clear["state"].as_str()? {
        "pending" => NativePreferenceClearState::Pending,
        "complete"
            if preferences["memory_only"] == false
                && clear.get("error").is_some_and(serde_json::Value::is_null) =>
        {
            NativePreferenceClearState::Complete
        }
        "failed" => NativePreferenceClearState::Failed,
        _ => return None,
    };
    let error = match clear.get("error") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(error)) => Some(error.clone()),
        _ => return None,
    };
    Some(NativePreferenceClearStatus {
        receipt: PreferenceClearReceipt {
            epoch: epoch.into(),
            id,
        },
        state,
        error,
    })
}

fn parse_status(response: &str) -> Result<NativeImeStatus, String> {
    if response.is_empty() {
        return Err("输入法设置请求未收到确认，请刷新状态后重试".into());
    }
    let value: serde_json::Value =
        serde_json::from_str(response).map_err(|_| "本机输入法服务版本过旧，请重新安装宿主")?;
    if value["ok"] != true {
        return Err(value["error"].as_str().unwrap_or("输入法设置未生效").into());
    }
    Ok(NativeImeStatus {
        settings: ImeSettings::from_json(&value["settings"].to_string())?,
        prediction: value["prediction"].as_str().unwrap_or("Disabled").into(),
        prediction_error: value["prediction_error"].as_str().map(str::to_string),
        preferences: NativePreferenceStatus {
            entries: value["preferences"]["entries"]
                .as_u64()
                .filter(|count| *count <= crate::preferences::MAX_ENTRIES as u64)
                .unwrap_or(0) as usize,
            enabled: value["preferences"]["enabled"].as_bool().unwrap_or(false),
            memory_only: value["preferences"]["memory_only"]
                .as_bool()
                .unwrap_or(true),
            pending: value["preferences"]["pending"].as_bool().unwrap_or(false),
            error: value["preferences"]["error"].as_str().map(str::to_string),
            clear: parse_preference_clear(&value["preferences"]),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::linux_ipc::tests::Endpoint;
    use std::{io::Write, time::Instant};

    #[test]
    fn combined_panel_binding_patch_is_not_reduced_to_a_legacy_model_toggle() {
        use crate::ime::{
            settings::PanelImeSettingsPatch,
            shortcuts::{ShortcutBindings, ShortcutProfile},
        };
        let bindings = ShortcutBindings::from_json(&serde_json::json!({
            "previous":"Ctrl+K", "next":"Ctrl+J", "previous-page":"Ctrl+H",
            "next-page":"Ctrl+L", "adopt":"Ctrl+Shift+A", "switch-language":"Alt+Shift+L"
        }))
        .unwrap();
        let patch = PanelImeSettingsPatch {
            enabled: Some(true),
            shortcut_bindings: Some(bindings),
            ..Default::default()
        };
        let command = panel_settings_command(patch).unwrap();
        assert!(command.starts_with('U'));
        assert!(command.len() > 128);
        assert!(command.len() <= 1024);
        assert_eq!(
            PanelImeSettingsPatch::from_json(&command[1..]).unwrap(),
            patch
        );
        for enabled in [false, true] {
            assert_eq!(
                panel_settings_command(PanelImeSettingsPatch {
                    enabled: Some(enabled),
                    ..Default::default()
                })
                .unwrap(),
                if enabled { "P1" } else { "P0" }
            );
            let clear = PanelImeSettingsPatch {
                enabled: Some(enabled),
                shortcut_bindings: Some(ShortcutBindings::default()),
                ..Default::default()
            };
            assert_eq!(
                panel_settings_command(clear).unwrap(),
                format!("U{}", clear.to_json())
            );
        }
        let invalid = PanelImeSettingsPatch {
            enabled: Some(true),
            shortcut_profile: Some(ShortcutProfile::HomeRow),
            shortcut_bindings: Some(
                ShortcutBindings::from_json(&serde_json::json!({"adopt":"Alt+J"})).unwrap(),
            ),
            ..Default::default()
        };
        assert!(panel_settings_command(invalid).is_err());
    }

    #[test]
    fn settings_connect_timeout_is_reported_without_a_late_request() {
        let endpoint = Endpoint::new();
        let _queued = endpoint.fill_backlog();
        let started = Instant::now();
        let error = request_at(&endpoint.path, "S", Duration::from_millis(45)).unwrap_err();
        assert!(error.contains("超时"));
        assert!(started.elapsed() < Duration::from_millis(300));
        drop(endpoint.accept());
        assert_eq!(
            endpoint.listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn trickled_settings_responses_use_one_budget_and_report_timeout_not_old_version() {
        let endpoint = Endpoint::new();
        let path = endpoint.path.clone();
        let server = std::thread::spawn(move || {
            let mut stream = endpoint.accept();
            let request = Deadline::new(Duration::from_millis(200))
                .read_to_end(&mut stream, 1)
                .unwrap();
            assert_eq!(request, b"S");
            stream
                .set_write_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            stream.set_nonblocking(false).unwrap();
            for _ in 0..30 {
                if stream.write_all(b" ").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let started = Instant::now();
        let error = request_at(&path, "S", Duration::from_millis(60)).unwrap_err();
        server.join().unwrap();
        assert!(error.contains("超时"));
        assert!(started.elapsed() < Duration::from_millis(250));
    }

    #[test]
    fn settings_response_byte_limit_accepts_exact_json_and_rejects_excess() {
        for size in [8192, 8193] {
            let endpoint = Endpoint::new();
            let path = endpoint.path.clone();
            let server = std::thread::spawn(move || {
                let mut stream = endpoint.accept();
                let deadline = Deadline::new(Duration::from_millis(500));
                assert_eq!(deadline.read_to_end(&mut stream, 1).unwrap(), b"S");
                let mut bytes = serde_json::json!({"ok":true,"settings":ImeSettings::default().to_json(),"prediction":"Pending"}).to_string().into_bytes();
                bytes.resize(size, b' ');
                let _ = deadline.send(&mut stream, &bytes);
            });
            let result = request_at(&path, "S", Duration::from_millis(500));
            server.join().unwrap();
            if size == 8192 {
                assert_eq!(result.unwrap().prediction, "Pending");
            } else {
                assert!(result.unwrap_err().contains("过大"));
            }
        }
    }
    #[test]
    fn parses_native_settings_and_rejects_legacy_or_failed_acknowledgements() {
        let raw = serde_json::json!({"ok":true,"settings":ImeSettings::default().to_json(),"prediction":"Pending"}).to_string();
        let parsed = parse_status(&raw).unwrap();
        assert_eq!(parsed.settings.language, BuiltinLanguage::English);
        assert_eq!(parsed.prediction, "Pending");
        assert!(parse_status("0").is_err());
        assert!(parse_status("1").is_err());
        assert_eq!(
            parse_status(r#"{"ok":false,"error":"not saved"}"#).unwrap_err(),
            "not saved"
        );
    }

    #[test]
    fn closed_settings_reply_is_unconfirmed_not_an_old_host() {
        let endpoint = Endpoint::new();
        let path = endpoint.path.clone();
        let server = std::thread::spawn(move || {
            let mut stream = endpoint.accept();
            assert_eq!(
                Deadline::new(Duration::from_millis(500))
                    .read_to_end(&mut stream, 3)
                    .unwrap(),
                b"Len"
            );
            // EOF without an acknowledgement cannot establish whether a save
            // happened; do not report an obsolete host or claim it was not saved.
        });
        let result = request_at(&path, "Len", Duration::from_millis(500));
        server.join().unwrap();
        assert_eq!(
            result.unwrap_err(),
            "输入法设置请求未收到确认，请刷新状态后重试"
        );
    }

    fn clear_reply(state: &str, ok: bool) -> serde_json::Value {
        serde_json::json!({"ok":ok, "error":if ok {None} else {Some("accepted, not completed")},
            "settings":ImeSettings::default().to_json(), "prediction":"Disabled",
            "preference_clear_accepted":7,
            "preferences":{"enabled":true,"memory_only":false,"pending":state=="pending",
                "entries":0,"epoch":"0123456789abcdef0123456789abcdef",
                "clear":{"id":7,"state":state,"error":null}}})
    }

    fn clear_server(
        replies: Vec<(Vec<u8>, String)>,
    ) -> (std::path::PathBuf, std::thread::JoinHandle<()>) {
        let endpoint = Endpoint::new();
        let path = endpoint.path.clone();
        let server = std::thread::spawn(move || {
            for (command, reply) in replies {
                let mut stream = endpoint.accept();
                let deadline = Deadline::new(Duration::from_millis(500));
                assert_eq!(deadline.read_to_end(&mut stream, 1).unwrap(), command);
                deadline.send(&mut stream, reply.as_bytes()).unwrap();
            }
        });
        (path, server)
    }

    #[test]
    fn file_clear_requires_matching_completed_receipt_and_never_repeats_f() {
        let (path, server) = clear_server(vec![
            (b"F".to_vec(), clear_reply("pending", false).to_string()),
            (b"S".to_vec(), clear_reply("pending", true).to_string()),
            (b"S".to_vec(), clear_reply("complete", true).to_string()),
        ]);
        let result = clear_preferences_at(&path, Duration::from_millis(400)).unwrap();
        server.join().unwrap();
        assert!(result.preferences.enabled);
        assert!(!result.preferences.memory_only);
        assert!(!result.preferences.pending);
    }

    #[test]
    fn clear_rejects_restarted_superseded_failed_or_incomplete_acknowledgements() {
        for variant in [
            "epoch", "id", "failed", "unknown", "error", "memory", "empty",
        ] {
            let mut reply = clear_reply("complete", true);
            match variant {
                "epoch" => {
                    reply["preferences"]["epoch"] = "fedcba9876543210fedcba9876543210".into()
                }
                "id" => reply["preferences"]["clear"]["id"] = 8.into(),
                "failed" => {
                    reply["preferences"]["clear"]["state"] = "failed".into();
                    reply["preferences"]["clear"]["error"] = "synthetic file failure".into();
                }
                "unknown" => reply["preferences"]["clear"]["state"] = "unknown".into(),
                "error" => reply["preferences"]["clear"]["error"] = "synthetic file failure".into(),
                "memory" => reply["preferences"]["memory_only"] = true.into(),
                _ => {}
            }
            let (path, server) = clear_server(vec![
                (b"F".to_vec(), clear_reply("pending", false).to_string()),
                (
                    b"S".to_vec(),
                    if variant == "empty" {
                        String::new()
                    } else {
                        reply.to_string()
                    },
                ),
            ]);
            let result = clear_preferences_at(&path, Duration::from_millis(400));
            server.join().unwrap();
            assert!(
                result.is_err(),
                "{variant} must never confirm disk deletion"
            );
        }
    }

    #[test]
    fn legacy_memory_clear_is_usable_but_disk_ok_without_receipt_is_not() {
        for enabled in [false, true] {
            let mut reply = clear_reply("complete", true);
            reply["preferences"]["enabled"] = enabled.into();
            let (path, server) = clear_server(vec![(b"F".to_vec(), reply.to_string())]);
            let result = clear_preferences_at(&path, Duration::from_millis(400));
            server.join().unwrap();
            assert_eq!(result.is_ok(), !enabled);
        }
        let (path, server) = clear_server(vec![(b"F".to_vec(), String::new())]);
        assert!(clear_preferences_at(&path, Duration::from_millis(400)).is_err());
        server.join().unwrap();
    }

    #[test]
    fn pending_clear_uses_one_deadline_and_reports_unconfirmed_without_resending() {
        let endpoint = Endpoint::new();
        let path = endpoint.path.clone();
        let server = std::thread::spawn(move || {
            let mut first = endpoint.accept();
            let deadline = Deadline::new(Duration::from_millis(500));
            assert_eq!(deadline.read_to_end(&mut first, 1).unwrap(), b"F");
            deadline
                .send(
                    &mut first,
                    clear_reply("pending", false).to_string().as_bytes(),
                )
                .unwrap();
            drop(first);
            let mut second = endpoint.accept();
            assert_eq!(deadline.read_to_end(&mut second, 1).unwrap(), b"S");
            std::thread::sleep(Duration::from_millis(90));
        });
        let started = Instant::now();
        let result = clear_preferences_at(&path, Duration::from_millis(45));
        let elapsed = started.elapsed();
        server.join().unwrap();
        assert!(result.unwrap_err().contains("未获确认"));
        assert!(elapsed < Duration::from_millis(200));
    }

    #[test]
    fn tracked_timeout_keeps_receipt_for_a_later_read_only_completion() {
        let endpoint = Endpoint::new();
        let path = endpoint.path.clone();
        let (release, released) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let mut first = endpoint.accept();
            let deadline = Deadline::new(Duration::from_millis(500));
            assert_eq!(deadline.read_to_end(&mut first, 1).unwrap(), b"F");
            deadline
                .send(
                    &mut first,
                    clear_reply("pending", false).to_string().as_bytes(),
                )
                .unwrap();
            drop(first);
            let mut pending = endpoint.accept();
            assert_eq!(deadline.read_to_end(&mut pending, 1).unwrap(), b"S");
            // The original client must return on its own budget before the host
            // publishes completion. A later status request cannot send F again.
            released.recv_timeout(Duration::from_millis(500)).unwrap();
            drop(pending);
            let mut refreshed = endpoint.accept();
            let deadline = Deadline::new(Duration::from_millis(500));
            assert_eq!(deadline.read_to_end(&mut refreshed, 1).unwrap(), b"S");
            deadline
                .send(
                    &mut refreshed,
                    clear_reply("complete", true).to_string().as_bytes(),
                )
                .unwrap();
        });
        let started = Instant::now();
        let failure = clear_preferences_tracked_at(&path, Duration::from_millis(45)).unwrap_err();
        assert!(failure.message.contains("未获确认"));
        assert!(started.elapsed() < Duration::from_millis(200));
        let receipt = failure.receipt.unwrap();
        assert_eq!(receipt.id, 7);
        assert_eq!(receipt.epoch, "0123456789abcdef0123456789abcdef");
        release.send(()).unwrap();
        let refreshed = request_at(&path, "S", Duration::from_millis(400)).unwrap();
        server.join().unwrap();
        let clear = refreshed.preferences.clear.unwrap();
        assert_eq!(clear.receipt, receipt);
        assert_eq!(clear.state, NativePreferenceClearState::Complete);
        assert_eq!(clear.error, None);
    }

    #[test]
    fn tracked_clear_does_not_invent_receipts_from_invalid_acceptance() {
        for variant in ["epoch", "id", "zero", "disabled", "missing"] {
            let mut reply = clear_reply("pending", false);
            match variant {
                "epoch" => reply["preferences"]["epoch"] = "not-a-valid-epoch".into(),
                "id" => reply["preference_clear_accepted"] = "7".into(),
                "zero" => reply["preference_clear_accepted"] = 0.into(),
                "disabled" => reply["preferences"]["enabled"] = false.into(),
                "missing" => {
                    reply
                        .as_object_mut()
                        .unwrap()
                        .remove("preference_clear_accepted");
                }
                _ => unreachable!(),
            }
            let (path, server) = clear_server(vec![(b"F".to_vec(), reply.to_string())]);
            let failure =
                clear_preferences_tracked_at(&path, Duration::from_millis(400)).unwrap_err();
            server.join().unwrap();
            assert_eq!(failure.receipt, None, "{variant}");
        }
        for variant in ["epoch", "id"] {
            let accepted = clear_reply("pending", false);
            let mut replaced = clear_reply("complete", true);
            if variant == "epoch" {
                replaced["preferences"]["epoch"] = "fedcba9876543210fedcba9876543210".into();
            } else {
                replaced["preferences"]["clear"]["id"] = 8.into();
            }
            let (path, server) = clear_server(vec![
                (b"F".to_vec(), accepted.to_string()),
                (b"S".to_vec(), replaced.to_string()),
            ]);
            let failure =
                clear_preferences_tracked_at(&path, Duration::from_millis(400)).unwrap_err();
            server.join().unwrap();
            assert!(failure.message.contains("已更换"));
            assert_eq!(
                failure.receipt,
                Some(PreferenceClearReceipt {
                    epoch: "0123456789abcdef0123456789abcdef".into(),
                    id: 7,
                }),
                "{variant} must retain the original, not the replacement operation"
            );
        }
    }

    #[test]
    fn refreshed_clear_requires_trustworthy_identity_and_completion_fields() {
        for variant in [
            "epoch",
            "id",
            "disabled",
            "unknown",
            "error",
            "missing-error",
            "memory",
            "missing-memory",
        ] {
            let mut reply = clear_reply("complete", true);
            match variant {
                "epoch" => {
                    reply["preferences"]["epoch"] = "z123456789abcdef0123456789abcdef".into()
                }
                "id" => reply["preferences"]["clear"]["id"] = 0.into(),
                "disabled" => reply["preferences"]["enabled"] = false.into(),
                "unknown" => reply["preferences"]["clear"]["state"] = "unknown".into(),
                "error" => reply["preferences"]["clear"]["error"] = "not synced".into(),
                "missing-error" => {
                    reply["preferences"]["clear"]
                        .as_object_mut()
                        .unwrap()
                        .remove("error");
                }
                "memory" => reply["preferences"]["memory_only"] = true.into(),
                "missing-memory" => {
                    reply["preferences"]
                        .as_object_mut()
                        .unwrap()
                        .remove("memory_only");
                }
                _ => unreachable!(),
            }
            assert_eq!(
                parse_status(&reply.to_string()).unwrap().preferences.clear,
                None,
                "{variant}"
            );
        }
        let mut pending = clear_reply("pending", true);
        pending["preferences"]["memory_only"] = true.into();
        let pending = parse_status(&pending.to_string())
            .unwrap()
            .preferences
            .clear
            .unwrap();
        assert_eq!(
            pending.state,
            NativePreferenceClearState::Pending,
            "a corrupt-file reset remains pending while the new store is created"
        );
        let mut failed = clear_reply("failed", true);
        failed["preferences"]["clear"]["error"] = "synthetic sync failure".into();
        let failed = parse_status(&failed.to_string())
            .unwrap()
            .preferences
            .clear
            .unwrap();
        assert_eq!(failed.state, NativePreferenceClearState::Failed);
        assert_eq!(failed.error.as_deref(), Some("synthetic sync failure"));
    }

    #[test]
    fn tracked_legacy_ram_clear_still_completes_without_a_disk_receipt() {
        let reply = serde_json::json!({"ok":true,"settings":ImeSettings::default().to_json()});
        let (path, server) = clear_server(vec![(b"F".to_vec(), reply.to_string())]);
        let result = clear_preferences_tracked_at(&path, Duration::from_millis(400)).unwrap();
        server.join().unwrap();
        assert!(!result.preferences.enabled);
        assert!(result.preferences.memory_only);
        assert_eq!(result.preferences.clear, None);
        let (path, server) = clear_server(vec![(b"F".to_vec(), String::new())]);
        let failure = clear_preferences_tracked_at(&path, Duration::from_millis(400)).unwrap_err();
        server.join().unwrap();
        assert_eq!(failure.receipt, None);
    }
}

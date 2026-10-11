//! User-only native host settings channel. No input text is involved in these requests.
use super::linux_ipc::Deadline;
use crate::{ime::settings::ImeSettings, languages::BuiltinLanguage};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeImeStatus {
    pub settings: ImeSettings,
    pub prediction: String,
    pub prediction_error: Option<String>,
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
    request("F")
}

fn request(command: &str) -> Result<NativeImeStatus, String> {
    let path = std::env::var_os("SUZAKU_LINUX_IME_SOCKET")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(|root| std::path::PathBuf::from(root).join("suzaku-ime/host.sock"))
        })
        .ok_or("无法定位本机输入法服务")?;
    request_at(&path, command, Duration::from_millis(700))
}

fn request_at(
    path: &std::path::Path,
    command: &str,
    timeout: Duration,
) -> Result<NativeImeStatus, String> {
    let deadline = Deadline::new(timeout);
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
    parse_status(&response)
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
}

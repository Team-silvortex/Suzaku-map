//! Presentation-only adapters for application-owned input-service diagnostics.
//!
//! Older/native producers retain their existing strings and wire format. Match
//! only explicit whole messages or the outer shell of a documented template;
//! an operating-system error or remote diagnostic inside that shell is opaque.

use super::UiLanguage;
use std::borrow::Cow;

// Canonical English resource key, followed by the legacy Chinese spelling.
// Keep this list independent of CLI/IPC producers and ordinary chrome labels.
const FIXED: &[(&str, &str)] = &[
    (
        "No previous input method is known; switch using the system input menu before quitting Suzaku",
        "无法确认可恢复的原输入法；请先从系统菜单切换其他输入法，再退出 Suzaku",
    ),
    (
        "Cannot confirm a safe fallback; the input service was kept running",
        "无法确认安全回退，已保留输入法服务",
    ),
    (
        "Input method did not switch; retry (previous input method retained)",
        "系统未完成输入法切换，请重试；原输入法已保留",
    ),
    (
        "Cannot identify the current input method; system settings were not changed",
        "无法确认当前输入法，未改动系统输入设置",
    ),
    (
        "IBus operation failed; check that the desktop input service is running",
        "IBus 操作失败，请确认桌面输入法服务正在运行",
    ),
    (
        "IBus returned an invalid input method name",
        "IBus 返回了无法识别的输入法名称",
    ),
    (
        "Input method switch timed out; retry (previous input method retained)",
        "输入法切换超时，请重试；原输入法已保留",
    ),
    (
        "Cannot start ibus; check that IBus is installed",
        "无法启动 ibus 命令，请确认 IBus 已安装",
    ),
    (
        "IBus response too large; request stopped",
        "IBus 返回数据过大，已停止请求",
    ),
    (
        "Cannot read IBus status; retry",
        "无法读取 IBus 状态，请重试",
    ),
    (
        "Suzaku input service is not registered; run suzaku-tool linux-register install first",
        "Suzaku 输入法服务尚未注册，请先运行 suzaku-tool linux-register install",
    ),
    (
        "Timed out waiting for the Suzaku input service to start or stop; retry and check service logs",
        "等待 Suzaku 输入法服务就绪或停止超时，请重试并检查服务日志",
    ),
    (
        "Suzaku user service operation failed; check systemctl --user status suzaku-ibus.service",
        "Suzaku 用户服务操作失败，请检查 systemctl --user status suzaku-ibus.service",
    ),
    (
        "User service returned an invalid status",
        "用户服务返回了无效状态",
    ),
    (
        "Cannot locate the local input service (XDG_RUNTIME_DIR is missing)",
        "无法定位本机输入法服务（缺少 XDG_RUNTIME_DIR）",
    ),
    (
        "Suzaku input service is not ready; retry",
        "Suzaku 输入法服务尚未就绪，请重试",
    ),
    (
        "Suzaku user service is masked; it was not unmasked automatically",
        "Suzaku 用户服务已被屏蔽，未擅自解除屏蔽",
    ),
    (
        "Cannot determine the Suzaku user service state; it was not forcibly started or stopped",
        "无法确认 Suzaku 用户服务状态，未强制启停",
    ),
    (
        "Cannot locate the local input service",
        "无法定位本机输入法服务",
    ),
    (
        "Input settings request timed out; retry",
        "输入法设置请求超时，请重试",
    ),
    ("Input service response too large", "输入法服务返回数据过大"),
    (
        "Suzaku input service is stopped or disconnected",
        "Suzaku 输入法服务未运行或连接已断开",
    ),
    (
        "Input service returned invalid text",
        "输入法服务返回了无效文本",
    ),
    (
        "Preference clear request was not acknowledged",
        "偏好清除请求未收到有效确认",
    ),
    (
        "Preference file clear was not acknowledged; refresh status",
        "偏好文件清除缺少操作确认，请刷新状态",
    ),
    (
        "Persistent preference clear was not acknowledged",
        "偏好清除未收到持久化操作确认",
    ),
    (
        "Invalid preference file clear status; success is unconfirmed",
        "偏好文件清除状态无效，未确认成功",
    ),
    (
        "Input service or clear task changed; the original operation is unconfirmed",
        "输入法宿主或清除任务已更换，未确认原操作成功",
    ),
    (
        "Incomplete preference file clear status; success is unconfirmed",
        "偏好文件清除状态不完整，未确认成功",
    ),
    (
        "Preference file clear failed; success is unconfirmed",
        "偏好文件清除失败，未确认成功",
    ),
    (
        "Input settings request was not acknowledged; refresh status and retry",
        "输入法设置请求未收到确认，请刷新状态后重试",
    ),
    (
        "Local input service is outdated; reinstall the host",
        "本机输入法服务版本过旧，请重新安装宿主",
    ),
    ("Input settings were not applied", "输入法设置未生效"),
    (
        "Input service recovered, but input source recovery failed; select an input method from the system menu",
        "输入法服务已恢复，但输入源恢复失败；请从系统菜单重新选择输入法",
    ),
];

// All templates have exactly one opaque slot. No template can recognize an
// arbitrary suffix, a partial sentence, a path, or a bare external error.
const TEMPLATES: &[(&str, &str)] = &[
    (
        "Cannot control the Suzaku user service: {}",
        "无法控制 Suzaku 用户服务：{}",
    ),
    (
        "Suzaku input service is stopped or disconnected: {}",
        "Suzaku 输入法服务未运行或连接已断开：{}",
    ),
    (
        "Preference file clear is unconfirmed; refresh status: {}",
        "偏好文件清除未获确认，请刷新状态：{}",
    ),
];

// PreferenceStore and PreferenceWriter expose only PreferenceError's bounded,
// parameter-free Display strings. These resources predate this adapter; keep
// their separate explicit allowlist instead of accepting all chrome labels.
const PREFERENCE_ERRORS: &[&str] = &[
    "Cannot obtain a preference fingerprint key",
    "Invalid preference document",
    "Unsupported preference format",
    "Preference file exceeds its size limit",
    "Preference staging exceeds its bounded recovery limit",
    "Preference restore timed out; using memory-only learning",
    "Cannot represent the preference timestamp",
    "Preference storage requires a private absolute file path",
    "Preference storage requires owned private regular files",
    "Preference storage is already in use",
    "Preference file changed externally; it was not overwritten",
    "Preference snapshot belongs to another cache",
    "An older preference snapshot cannot replace newer state",
    "Only an empty preference snapshot may clear the file",
    "Preference restore requires an unused empty input context",
    "Preferences cannot be restored or exported in a private context",
    "Preference operation expired or was superseded before publication",
    "Preference file operation failed; publication may be unconfirmed",
];

// Native host status retains PredictionError::Display's existing wire strings.
// Recognize each bounded variant explicitly, without parsing remote errors or
// inventing an HTTP status number that Display did not preserve.
const PREDICTION_ERRORS: &[(&str, &str)] = &[
    (
        "Prediction request cancelled; local input is unaffected",
        "模型请求已取消，本地输入不受影响",
    ),
    (
        "Prediction request invalid or over budget; local candidates retained",
        "预测请求无效或超出预算，已保留本地候选",
    ),
    (
        "Prediction protocol version not supported; local candidates retained",
        "预测协议版本不受支持，已保留本地候选",
    ),
    (
        "Invalid model settings: local requires loopback HTTP; cloud requires HTTPS and an explicit model",
        "模型配置无效：本地需回环 HTTP，云端需 HTTPS 和明确模型",
    ),
    (
        "Model service unavailable or secure connection failed; local candidates remain available",
        "模型服务不可用或安全连接失败，本地候选仍可使用",
    ),
    (
        "Prediction request timed out; local candidates remain available",
        "模型请求超时，本地候选仍可使用",
    ),
    (
        "Model service refused authorization; check credentials and permissions",
        "模型服务拒绝授权，请检查密钥与访问权限",
    ),
    (
        "Model or endpoint does not exist; check model settings",
        "模型或接口不存在，请检查模型配置",
    ),
    (
        "Model service returned an error; check service status",
        "模型服务返回错误，请检查服务状态",
    ),
    (
        "Invalid model response format; local candidates retained",
        "模型响应格式无效，已保留本地候选",
    ),
    (
        "Model response too large; processing refused",
        "模型响应过大，已拒绝处理",
    ),
    (
        "Model returned no valid candidates; local candidates retained",
        "模型未返回有效候选，已保留本地候选",
    ),
    (
        "No local model available; start a local service or select a model; cloud was not contacted",
        "未发现可用本机模型，请启动本地服务或指定模型；未访问云端",
    ),
    (
        "Cloud prediction is not authorized; input will not be sent to the cloud",
        "云端联想尚未授权，输入内容不会发送到云端",
    ),
    (
        "Model credential environment variable is missing or invalid",
        "模型密钥环境变量未设置或格式无效",
    ),
];

const TRAY_CONTROLLER_ERROR: &str = "Tray controller unavailable; restart Suzaku";

pub(super) fn diagnostic<'a>(language: UiLanguage, source: &'a str) -> Cow<'a, str> {
    for &(key, legacy) in FIXED.iter().chain(PREDICTION_ERRORS) {
        if source == key || source == legacy {
            return Cow::Borrowed(language.tr(key));
        }
    }
    if let Some(&key) = PREFERENCE_ERRORS.iter().find(|&&key| source == key) {
        return Cow::Borrowed(language.tr(key));
    }
    if source == TRAY_CONTROLLER_ERROR {
        return Cow::Borrowed(language.tr(TRAY_CONTROLLER_ERROR));
    }
    for &(key, legacy) in TEMPLATES {
        for template in [key, legacy] {
            let (prefix, suffix) = template.split_once("{}").expect("one-slot diagnostic");
            if let Some(payload) = source
                .strip_prefix(prefix)
                .and_then(|tail| tail.strip_suffix(suffix))
            {
                // Never pass this payload to message(), diagnostic(), or tr().
                return Cow::Owned(language.tr(key).replace("{}", payload));
            }
        }
    }
    // A remote diagnostic may coincidentally look like an ordinary UI label.
    // Do not use the general chrome-message matcher as a fallback here.
    Cow::Borrowed(source)
}

#[cfg(test)]
pub(super) fn known_keys() -> impl Iterator<Item = &'static str> {
    FIXED
        .iter()
        .chain(TEMPLATES)
        .chain(PREDICTION_ERRORS)
        .map(|&(key, _)| key)
        .chain(PREFERENCE_ERRORS.iter().copied())
        .chain(std::iter::once(TRAY_CONTROLLER_ERROR))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn explicit_languages() -> impl Iterator<Item = UiLanguage> {
        UiLanguage::ALL
            .into_iter()
            .filter(|language| *language != UiLanguage::System)
    }

    #[test]
    fn explicit_diagnostic_keys_are_unique_complete_and_have_one_or_no_slots() {
        let keys: std::collections::HashSet<_> = known_keys().collect();
        assert_eq!(keys.len(), 71);
        assert_eq!(
            keys.len(),
            FIXED.len() + TEMPLATES.len() + PREDICTION_ERRORS.len() + PREFERENCE_ERRORS.len() + 1
        );
        for key in known_keys() {
            assert!(
                super::super::catalog().contains_key(key),
                "missing diagnostic resource: {key}"
            );
            assert!(key.matches("{}").count() <= 1);
            for language in explicit_languages() {
                if language != UiLanguage::English {
                    assert_ne!(language.tr(key), key, "untranslated {language:?}/{key}");
                }
            }
        }
        for &(key, legacy) in TEMPLATES {
            assert_eq!(key.matches("{}").count(), 1);
            assert_eq!(legacy.matches("{}").count(), 1);
            assert!(!key.split_once("{}").unwrap().0.is_empty());
            assert!(!legacy.split_once("{}").unwrap().0.is_empty());
        }
    }

    #[test]
    fn current_and_legacy_fixed_diagnostics_use_the_selected_interface_language() {
        for language in explicit_languages() {
            for &(key, legacy) in FIXED {
                assert_eq!(
                    diagnostic(language, key),
                    language.tr(key),
                    "{language:?}/{key}"
                );
                assert_eq!(
                    diagnostic(language, legacy),
                    language.tr(key),
                    "{language:?}/{legacy}"
                );
            }
        }
        assert_eq!(
            diagnostic(UiLanguage::English, "输入法设置请求超时，请重试"),
            "Input settings request timed out; retry"
        );
    }

    #[test]
    fn preference_storage_display_errors_use_only_the_existing_explicit_resources() {
        use crate::preferences::persistence::PreferenceError;

        let errors = [
            PreferenceError::EntropyUnavailable,
            PreferenceError::InvalidDocument,
            PreferenceError::UnsupportedFormat,
            PreferenceError::FileTooLarge,
            PreferenceError::StagingLimit,
            PreferenceError::StartupTimeout,
            PreferenceError::ClockUnavailable,
            PreferenceError::InvalidPath,
            PreferenceError::InsecureFile,
            PreferenceError::Busy,
            PreferenceError::Conflict,
            PreferenceError::WrongCache,
            PreferenceError::StaleSnapshot,
            PreferenceError::NotEmpty,
            PreferenceError::InputBusy,
            PreferenceError::PrivateContext,
            PreferenceError::Cancelled,
            PreferenceError::Io(std::io::ErrorKind::PermissionDenied),
        ];
        assert_eq!(errors.len(), PREFERENCE_ERRORS.len());
        for (error, &key) in errors.into_iter().zip(PREFERENCE_ERRORS) {
            let source = error.to_string();
            assert_eq!(source, key, "preference Display resource drift");
            for language in explicit_languages() {
                assert_eq!(diagnostic(language, &source), language.tr(key));
                for unknown in [format!("{source}: Settings"), format!("prefix{source}")] {
                    assert_eq!(diagnostic(language, &unknown), unknown);
                }
            }
        }
        // Generic I/O details are not themselves resource keys or path guesses.
        assert_eq!(
            PreferenceError::Io(std::io::ErrorKind::NotFound).to_string(),
            PREFERENCE_ERRORS[PREFERENCE_ERRORS.len() - 1]
        );
        for language in explicit_languages() {
            for source in ["Settings", "Font: Settings", "custom settings"] {
                assert_eq!(diagnostic(language, source), source);
            }
        }
    }

    #[test]
    fn unavailable_tray_controller_is_an_exact_owned_diagnostic() {
        for language in explicit_languages() {
            assert_eq!(
                diagnostic(language, TRAY_CONTROLLER_ERROR),
                language.tr(TRAY_CONTROLLER_ERROR)
            );
            for unknown in [
                format!("{TRAY_CONTROLLER_ERROR}: Settings"),
                format!("prefix{TRAY_CONTROLLER_ERROR}"),
            ] {
                assert_eq!(diagnostic(language, &unknown), unknown);
            }
        }
    }

    #[test]
    fn native_prediction_display_errors_have_explicit_resources_without_http_status_guessing() {
        use crate::prediction::PredictionError;

        let cases = [
            (PredictionError::Cancelled, 0),
            (PredictionError::InvalidRequest, 1),
            (PredictionError::UnsupportedProtocol, 2),
            (PredictionError::InvalidEndpoint, 3),
            (PredictionError::Unavailable, 4),
            (PredictionError::Timeout, 5),
            (PredictionError::HttpStatus(401), 6),
            (PredictionError::HttpStatus(403), 6),
            (PredictionError::HttpStatus(404), 7),
            (PredictionError::HttpStatus(429), 8),
            (PredictionError::HttpStatus(500), 8),
            (PredictionError::InvalidResponse, 9),
            (PredictionError::ResponseTooLarge, 10),
            (PredictionError::NoCandidates, 11),
            (PredictionError::NoLocalModel, 12),
            (PredictionError::CloudConsentRequired, 13),
            (PredictionError::MissingCredentials, 14),
        ];
        for (error, index) in cases {
            let (key, legacy) = PREDICTION_ERRORS[index];
            let raw = error.to_string();
            assert_eq!(raw, legacy, "prediction Display resource drift");
            for language in explicit_languages() {
                assert_eq!(diagnostic(language, &raw), language.tr(key));
                assert_eq!(diagnostic(language, key), language.tr(key));
                for unknown in [format!("{raw}: Settings"), format!("prefix{raw}")] {
                    assert_eq!(diagnostic(language, &unknown), unknown);
                }
                if let PredictionError::HttpStatus(_) = error {
                    assert!(
                        !diagnostic(language, &raw)
                            .chars()
                            .any(|ch| ch.is_ascii_digit()),
                        "a display-only diagnostic must not invent an HTTP number"
                    );
                }
            }
        }
        for language in explicit_languages() {
            let payload = PredictionError::Timeout.to_string();
            let source = format!("偏好文件清除未获确认，请刷新状态：{payload}");
            assert_eq!(
                diagnostic(language, &source),
                language
                    .tr("Preference file clear is unconfirmed; refresh status: {}")
                    .replace("{}", &payload),
                "even a known nested prediction error remains an opaque payload"
            );
        }
    }

    #[test]
    fn diagnostic_template_payloads_remain_exactly_opaque_in_all_eight_interfaces() {
        for language in explicit_languages() {
            for &(key, legacy) in TEMPLATES {
                for payload in [
                    "",
                    "Settings",
                    "Font: Settings",
                    "{}",
                    "{}_日_ß_Ö",
                    "/home/用户/Font: Settings_{}.json",
                    "http://127.0.0.1:11434/v1/models?id=Settings_{}",
                    "Permission denied (os error 13)",
                    "输入法设置请求超时，请重试",
                    "Cannot control the Suzaku user service: Settings",
                ] {
                    let expected = language.tr(key).replace("{}", payload);
                    for source in [key.replace("{}", payload), legacy.replace("{}", payload)] {
                        assert_eq!(
                            diagnostic(language, &source),
                            expected,
                            "{language:?}/{source}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn unknown_remote_diagnostics_are_not_interpreted_as_chrome_messages() {
        for language in explicit_languages() {
            for source in [
                "Font: Settings",
                "Settings",
                "custom settings",
                "Candidate preferences: Settings",
                "service error: 输入法设置未生效",
                "prefix输入法设置请求超时，请重试",
                "输入法设置请求超时，请重试 suffix",
                "输入法设置未生效\nSettings",
                "prefixCannot control the Suzaku user service: Settings",
                "打开目录失败：Settings",
                "Permission denied (os error 13)",
                "/home/用户/Font: Settings_{}.json",
            ] {
                assert_eq!(
                    diagnostic(language, source),
                    source,
                    "{language:?}/{source}"
                );
                assert!(matches!(diagnostic(language, source), Cow::Borrowed(_)));
            }
        }
    }

    #[test]
    fn unconfirmed_clear_only_localizes_the_outer_operation_not_nested_errors() {
        let payload = "Suzaku 输入法服务未运行或连接已断开：Font: Settings_{}";
        let source = format!("偏好文件清除未获确认，请刷新状态：{payload}");
        for language in explicit_languages() {
            let key = "Preference file clear is unconfirmed; refresh status: {}";
            assert_eq!(
                diagnostic(language, &source),
                language.tr(key).replace("{}", payload)
            );
        }
        assert_eq!(
            diagnostic(UiLanguage::English, &source),
            format!("Preference file clear is unconfirmed; refresh status: {payload}")
        );
    }
}

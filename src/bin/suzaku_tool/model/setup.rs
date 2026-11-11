//! Explicit offline terminal setup. Credentials are references, never key values.
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::Path;
use suzaku_map::ime::settings::ImeSettings;
use suzaku_map::model::{
    DEFAULT_LOCAL_ENDPOINT, DEFAULT_MODEL, ModelProtocol, ModelProviderConfig, ModelScope,
    openai_compatible_endpoint_from_base_url,
};

const MAX_INPUT_BYTES: usize = 2048;

pub(super) fn ensure_terminal() -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(
            "model setup requires an interactive terminal on stdin and stdout; no settings changed."
                .into(),
        );
    }
    Ok(())
}

pub(super) fn run(current: ImeSettings) -> Result<(), String> {
    ensure_terminal()?;
    let path = suzaku_map::ui::model_setup::ModelSetupGuide::current()
        .settings_path
        .ok_or("Cannot resolve this process's configuration path; no settings changed.")?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_with(
        current,
        &path,
        &mut stdin.lock(),
        &mut stdout.lock(),
        |next, expected| next.save_if_unchanged(expected),
    )
}

fn terminal_error(_: io::Error) -> String {
    "Terminal I/O failed; check the saved configuration before retrying.".into()
}

fn destination(output: &mut impl Write, path: &Path) -> Result<(), String> {
    writeln!(
        output,
        "Destination (this process): {}",
        path.to_string_lossy().escape_debug()
    )
    .map_err(terminal_error)
}

fn run_with(
    current: ImeSettings,
    path: &Path,
    input: &mut impl BufRead,
    output: &mut impl Write,
    save: impl FnOnce(&ImeSettings, &ImeSettings) -> Result<(), String>,
) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Cannot resolve an absolute configuration path; no settings changed.".into());
    }
    writeln!(
        output,
        "Offline model setup. No network requests or credential values are read.\nType :cancel at any prompt to cancel. Enter accepts a displayed default.\nNever enter an API key: only its environment variable NAME belongs here.\nSaving clears cloud consent in the file; running processes are not reloaded."
    )
    .map_err(terminal_error)?;
    destination(output, path)?;
    let Some(next) = collect_settings(&current, input, output)? else {
        return cancelled(output);
    };
    writeln!(output, "Pending preview (not saved):").map_err(terminal_error)?;
    destination(output, path)?;
    writeln!(
        output,
        "scope: {}\nprotocol: {}\nendpoint: {}\nmodel: {}\ncredential environment name: {}\ntimeout: {} ms\ncloud consent in saved file: false\nLLM enabled (preserved): {}\ntemperature tenths (preserved): {}",
        next.provider.scope.id(),
        next.provider.protocol.id(),
        next.provider.endpoint.escape_debug(),
        next.provider.model.escape_debug(),
        next.provider.api_key_env.as_deref().unwrap_or("none").escape_debug(),
        next.provider.timeout_ms,
        next.llm_enabled,
        next.provider.temperature_tenths,
    )
    .map_err(terminal_error)?;
    let confirmation = answer(
        input,
        output,
        "Type save to save; anything else cancels: ",
        None,
    )?;
    if confirmation.as_deref() != Some("save") {
        return cancelled(output);
    }
    save(&next, &current)
        .map_err(|error| format!("Save was not confirmed: {}", error.escape_debug()))?;
    writeln!(
        output,
        "Saved configuration with cloud consent disabled; other input preferences were preserved.\nNo host was reloaded, no LLM was enabled, and no credential value or model was accessed.\nSupply credentials separately to the actual host and panel processes. For cloud use, explicitly authorize the chosen target, then reload model settings in the tray. Enabling LLM is separate."
    )
    .map_err(terminal_error)
}

fn cancelled(output: &mut impl Write) -> Result<(), String> {
    writeln!(output, "Setup cancelled; settings unchanged.").map_err(terminal_error)
}

fn answer(
    input: &mut impl BufRead,
    output: &mut impl Write,
    prompt: &str,
    default: Option<&str>,
) -> Result<Option<String>, String> {
    write!(output, "{prompt}").map_err(terminal_error)?;
    output.flush().map_err(terminal_error)?;
    let mut bytes = Vec::new();
    // A valid line may contain MAX_INPUT_BYTES followed by CRLF. Never allocate
    // an unbounded line, and never accept a partial line delivered by EOF.
    let count = input
        .take((MAX_INPUT_BYTES + 2) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(terminal_error)?;
    if count == 0 {
        return Ok(None);
    }
    if bytes.last() != Some(&b'\n') {
        return if bytes.len() > MAX_INPUT_BYTES {
            Err("Setup input is too long; no settings changed.".into())
        } else {
            Ok(None)
        };
    }
    bytes.pop();
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("Setup input is too long; no settings changed.".into());
    }
    let value = String::from_utf8(bytes)
        .map_err(|_| "Setup input must be valid UTF-8; no settings changed.")?;
    if value.chars().any(char::is_control) {
        return Err("Setup input contains control characters; no settings changed.".into());
    }
    if value == ":cancel" {
        return Ok(None);
    }
    Ok(Some(if value.is_empty() {
        default.unwrap_or_default().to_owned()
    } else {
        value
    }))
}

fn field(
    input: &mut impl BufRead,
    output: &mut impl Write,
    label: &str,
    default: Option<&str>,
) -> Result<Option<String>, String> {
    let displayed = default.map_or_else(
        || "required".into(),
        |value| value.escape_debug().to_string(),
    );
    answer(
        input,
        output,
        &format!("{label} (Enter={displayed}): "),
        default,
    )
}

fn collect_settings(
    current: &ImeSettings,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Option<ImeSettings>, String> {
    let Some(scope) = answer(
        input,
        output,
        &format!(
            "Scope [local/cloud] (Enter={}): ",
            current.provider.scope.id()
        ),
        Some(current.provider.scope.id()),
    )?
    else {
        return Ok(None);
    };
    let scope = match scope.as_str() {
        "local" => ModelScope::Local,
        "cloud" => ModelScope::Cloud,
        _ => return Err("Invalid scope; no settings changed.".into()),
    };
    let Some(mode) = answer(
        input,
        output,
        "Address mode [endpoint/base] (Enter=endpoint): ",
        Some("endpoint"),
    )?
    else {
        return Ok(None);
    };
    if !matches!(mode.as_str(), "endpoint" | "base") {
        return Err("Invalid address mode; no settings changed.".into());
    }
    let unchanged_scope = scope == current.provider.scope;
    let mut protocol = if unchanged_scope {
        current.provider.protocol
    } else if scope == ModelScope::Local {
        ModelProtocol::Auto
    } else {
        ModelProtocol::OpenAiCompatible
    };
    if mode == "base"
        && !matches!(
            protocol,
            ModelProtocol::OpenAiCompatible
                | ModelProtocol::ChatCompletions
                | ModelProtocol::ChatCompletionsReasoning
        )
    {
        protocol = ModelProtocol::OpenAiCompatible;
    }
    let Some(protocol) = answer(
        input,
        output,
        &format!(
            "Protocol [auto/ollama/openai-compatible/chat-completions/chat-completions-low] (Enter={}): ",
            protocol.id()
        ),
        Some(protocol.id()),
    )?
    else {
        return Ok(None);
    };
    let protocol = match protocol.as_str() {
        "auto" => ModelProtocol::Auto,
        "ollama" => ModelProtocol::Ollama,
        "openai-compatible" => ModelProtocol::OpenAiCompatible,
        "chat-completions" => ModelProtocol::ChatCompletions,
        "chat-completions-low" => ModelProtocol::ChatCompletionsReasoning,
        _ => return Err("Invalid protocol; no settings changed.".into()),
    };
    if mode == "base"
        && !matches!(
            protocol,
            ModelProtocol::OpenAiCompatible
                | ModelProtocol::ChatCompletions
                | ModelProtocol::ChatCompletionsReasoning
        )
    {
        return Err(
            "API base mode requires a compatible chat protocol; no settings changed.".into(),
        );
    }
    let default_endpoint = if mode == "base" {
        None
    } else if unchanged_scope {
        Some(current.provider.endpoint.as_str())
    } else if scope == ModelScope::Local {
        Some(DEFAULT_LOCAL_ENDPOINT)
    } else {
        None
    };
    let Some(address) = field(
        input,
        output,
        if mode == "base" {
            "API base"
        } else {
            "Endpoint"
        },
        default_endpoint,
    )?
    else {
        return Ok(None);
    };
    let endpoint = if mode == "base" {
        openai_compatible_endpoint_from_base_url(&address, scope)
            .map_err(|_| "Invalid API base; no settings changed.")?
    } else {
        address
    };
    let mut checked = ModelProviderConfig {
        scope,
        protocol,
        endpoint,
        model: "setup-validation-only".into(),
        ..Default::default()
    };
    checked
        .validate()
        .map_err(|_| "Invalid endpoint; no settings changed.")?;
    let default_model = if unchanged_scope {
        Some(current.provider.model.as_str())
    } else if scope == ModelScope::Local {
        Some(DEFAULT_MODEL)
    } else {
        None
    };
    let Some(model) = field(input, output, "Model ID", default_model)? else {
        return Ok(None);
    };
    checked.model = model;
    checked
        .validate()
        .map_err(|_| "Invalid model ID; no settings changed.")?;
    let default_key = if unchanged_scope {
        current.provider.api_key_env.as_deref().unwrap_or("none")
    } else {
        "none"
    };
    let Some(key) = field(
        input,
        output,
        "Credential environment name",
        Some(default_key),
    )?
    else {
        return Ok(None);
    };
    checked.api_key_env = (key != "none").then_some(key);
    checked
        .validate()
        .map_err(|_| "Invalid credential environment name; no settings changed.")?;
    let timeout = current.provider.timeout_ms.to_string();
    let Some(timeout) = field(input, output, "Timeout ms", Some(&timeout))? else {
        return Ok(None);
    };
    let timeout = timeout
        .parse::<u64>()
        .ok()
        .filter(|value| (20..=5000).contains(value))
        .ok_or("Invalid timeout; use 20..5000 ms. No settings changed.")?;
    let options = [
        "--scope".into(),
        scope.id().into(),
        "--protocol".into(),
        protocol.id().into(),
        "--endpoint".into(),
        checked.endpoint,
        "--model".into(),
        checked.model,
        "--api-key-env".into(),
        checked.api_key_env.unwrap_or_else(|| "none".into()),
        "--timeout-ms".into(),
        timeout.to_string(),
        "--cloud-consent".into(),
        "false".into(),
    ];
    super::configured_settings(current.clone(), &options)
        .map(Some)
        .map_err(|_| "Invalid setup configuration; no settings changed.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, io::Cursor};

    fn fixture() -> ImeSettings {
        ImeSettings::from_json(r#"{"language":"ja","llm_enabled":true,"shortcut_profile":"home-row","llm_timeout_ms":4321,"llm_temperature_tenths":7}"#).unwrap()
    }

    fn exercise(
        current: ImeSettings,
        answers: &[u8],
        save: impl FnOnce(&ImeSettings, &ImeSettings) -> Result<(), String>,
    ) -> (Result<(), String>, String) {
        let mut output = Vec::new();
        let result = run_with(
            current,
            &std::env::current_dir()
                .unwrap()
                .join("SYNTHETIC/settings.json"),
            &mut Cursor::new(answers),
            &mut output,
            save,
        );
        (result, String::from_utf8(output).unwrap())
    }

    #[test]
    fn only_exact_confirmed_save_calls_the_adapter_once_and_preserves_other_preferences() {
        let current = fixture();
        let expected = current.clone();
        let count = Cell::new(0);
        let (result, output) = exercise(current, b"\n\n\n\n\n\n\nsave\n", |next, baseline| {
            count.set(count.get() + 1);
            assert_eq!(baseline, &expected);
            assert_eq!(next, &expected);
            Ok(())
        });
        result.unwrap();
        assert_eq!(count.get(), 1);
        assert!(output.contains("Pending preview (not saved)"));
        assert!(output.contains("Saved configuration with cloud consent disabled"));
        assert_eq!(output.matches("Destination (this process):").count(), 2);
    }

    #[test]
    fn cancellation_and_eof_at_every_stage_never_save() {
        for stage in 0..8 {
            for tail in [":cancel\n", ""] {
                let input = format!("{}{tail}", "\n".repeat(stage));
                let (result, output) = exercise(fixture(), input.as_bytes(), |_, _| {
                    panic!("cancel/EOF must not save")
                });
                result.unwrap();
                assert!(output.contains("Setup cancelled; settings unchanged."));
            }
        }
        for confirmation in ["save", "Save\n", " save\n", "save \n", "no\n"] {
            let input = format!("{}{confirmation}", "\n".repeat(7));
            let (result, output) = exercise(fixture(), input.as_bytes(), |_, _| {
                panic!("partial or non-exact save must not save")
            });
            result.unwrap();
            assert!(output.contains("Setup cancelled"));
        }
    }

    #[test]
    fn scope_changes_use_safe_local_defaults_and_always_revoke_cloud_consent() {
        let cloud = ImeSettings::from_json(r#"{"language":"ja","llm_enabled":true,"llm_scope":"cloud","llm_protocol":"chat-completions","llm_endpoint":"https://models.example/v1/chat/completions","llm_model":"arbitrary-model","llm_api_key_env":"SYNTHETIC_ENV_NAME","llm_cloud_consent":true,"llm_timeout_ms":4321,"llm_temperature_tenths":7}"#).unwrap();
        let (result, _) = exercise(cloud, b"local\n\n\n\n\n\n\nsave\n", |next, baseline| {
            assert_eq!(next.provider.scope, ModelScope::Local);
            assert_eq!(next.provider.protocol, ModelProtocol::Auto);
            assert_eq!(next.provider.endpoint, DEFAULT_LOCAL_ENDPOINT);
            assert_eq!(next.provider.model, DEFAULT_MODEL);
            assert_eq!(next.provider.api_key_env, None);
            assert!(!next.provider.cloud_consent);
            assert_eq!(next.provider.timeout_ms, baseline.provider.timeout_ms);
            assert_eq!(
                next.provider.temperature_tenths,
                baseline.provider.temperature_tenths
            );
            assert_eq!(next.language, baseline.language);
            assert_eq!(next.llm_enabled, baseline.llm_enabled);
            Ok(())
        });
        result.unwrap();
        let (result, _) = exercise(fixture(), b"cloud\n\n\n\n", |_, _| {
            panic!("a new cloud target must be explicit")
        });
        assert!(result.is_err());
    }

    #[test]
    fn cloud_base_setup_is_offline_and_does_not_retain_old_authorization() {
        let (result, _) = exercise(fixture(), b"cloud\nbase\nchat-completions\nhttps://models.example/gateway/v1/\narbitrary-model\nSYNTHETIC_ENV_NAME\n2500\nsave\n", |next, baseline| {
            assert_eq!(next.provider.endpoint, "https://models.example/gateway/v1/chat/completions");
            assert_eq!(next.provider.protocol, ModelProtocol::ChatCompletions);
            assert_eq!(next.provider.api_key_env.as_deref(), Some("SYNTHETIC_ENV_NAME"));
            assert!(!next.provider.cloud_consent);
            assert_eq!(next.llm_enabled, baseline.llm_enabled);
            Ok(())
        });
        result.unwrap();
    }

    #[test]
    fn invalid_or_oversized_answers_fail_without_repeating_the_value_or_saving() {
        for input in [
            b"SYNTHETIC_INVALID_SCOPE\n".to_vec(),
            b"local\n\n\n\n\nSYNTHETIC INVALID NAME\n".to_vec(),
            b"\x1bSYNTHETIC_CONTROL\n".to_vec(),
            vec![b'x'; MAX_INPUT_BYTES + 1],
            vec![0xff, b'\n'],
        ] {
            let (result, output) = exercise(fixture(), &input, |_, _| {
                panic!("invalid input must not save")
            });
            assert!(result.is_err());
            assert!(!output.contains("SYNTHETIC_INVALID_SCOPE"));
            assert!(!output.contains("SYNTHETIC INVALID NAME"));
            assert!(!output.contains("SYNTHETIC_CONTROL"));
        }
    }

    #[test]
    fn failed_save_is_not_reported_as_success_or_retried() {
        let count = Cell::new(0);
        let (result, output) = exercise(fixture(), b"\n\n\n\n\n\n\nsave\n", |_, _| {
            count.set(count.get() + 1);
            Err("synthetic concurrent edit".into())
        });
        assert!(result.unwrap_err().contains("Save was not confirmed"));
        assert_eq!(count.get(), 1);
        assert!(!output.contains("Saved configuration"));
    }

    #[test]
    fn path_display_is_escaped_without_changing_the_destination_or_reading_it() {
        let mut output = Vec::new();
        let path = std::env::current_dir()
            .unwrap()
            .join("SYNTHETIC/\x1b[31m\nsettings.json");
        run_with(
            fixture(),
            &path,
            &mut Cursor::new(b":cancel\n"),
            &mut output,
            |_, _| panic!("cancel must not save"),
        )
        .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(&path.to_string_lossy().escape_debug().to_string()));
        assert!(!output.contains('\x1b'));
    }
}

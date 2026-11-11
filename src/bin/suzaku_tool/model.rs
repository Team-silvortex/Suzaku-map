use std::time::Instant;
use suzaku_map::ime::{
    EngineConfig, XRTabletImeEngine,
    settings::{ImeSettings, settings_path},
};
use suzaku_map::languages::BuiltinLanguage;
use suzaku_map::model::{
    DEFAULT_LOCAL_ENDPOINT, DEFAULT_MODEL, HttpModelProvider, ModelScope, model_credential_status,
    openai_compatible_endpoint_from_base_url, runtime,
};
use suzaku_map::prediction::{
    PredictionCancellation, PredictionError, PredictionInput, PredictionProvider, PredictionRequest,
};

#[path = "model/setup.rs"]
mod setup;

const USAGE: &str = "Usage: model status | discover | warmup | help
  model paths | guide                         No configuration contents read
  model setup                                 Offline terminal configuration wizard
  model check [--credentials] [configuration options]
  model configure [--dry-run] [configuration options]
  model probe [all|en|zh-Hans|ja] [--once] [configuration options]
Configuration options (separate flag/value pairs; never pass a literal key):
  --scope local|cloud  --protocol auto|ollama|openai-compatible|chat-completions|chat-completions-low
  --model auto|NAME
  --endpoint FULL_GENERATION_URL | --base-url OPENAI_COMPATIBLE_API_BASE
  --timeout-ms 20..5000  --api-key-env NAME|none  --cloud-consent true|false
--base-url appends /chat/completions; it never guesses /v1.
chat-completions opts into max_completion_tokens + reasoning_effort=none.
chat-completions-low opts into reasoning_effort=low without sampling parameters.
Both profiles keep the same total token budget, including reasoning tokens.
check validates syntax only; credentials and service access are not checked.
check --credentials explicitly checks only this process's key presence/format.
It never authenticates, accesses a keyring, or checks the IME host environment.
setup requires terminal input/output; preview then type save to persist.
setup never grants cloud consent, reads key values, or tests a service.
configure without options resets to local autodiscovery, without enabling LLM.
probe uses synthetic text and may incur cloud charges; it never saves overrides.
--once defaults to English and issues at most one generation request, without warmup.
Explicit compatible model IDs avoid discovery; local Ollama still checks its inventory.
Target changes revoke old cloud consent.
The IME host must separately receive the named credential environment variable.";

pub(super) fn run(args: &[String]) -> i32 {
    match dispatch(args) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Model: {error}");
            1
        }
    }
}

fn dispatch(args: &[String]) -> Result<(), String> {
    if matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h")
    ) {
        if args.len() != 1 {
            return Err(USAGE.into());
        }
        println!("{USAGE}");
        return Ok(());
    }
    match args.first().map(String::as_str) {
        Some("setup") if args.len() == 1 => {
            setup::ensure_terminal()?;
            return setup::run(ImeSettings::load()?);
        }
        Some("setup") => return Err(USAGE.into()),
        Some("paths") if args.len() == 1 => return print_paths(),
        Some("guide") if args.len() == 1 => {
            println!(
                "{}",
                suzaku_map::ui::model_setup::ModelSetupGuide::current()
                    .render(suzaku_map::ui::UiLanguage::System)
                    .escape_debug()
            );
            return Ok(());
        }
        Some("paths" | "guide") => return Err(USAGE.into()),
        _ => {}
    }
    let mut settings = ImeSettings::load()?;
    match args.first().map(String::as_str).unwrap_or("status") {
        "status" if args.len() <= 1 => {
            println!(
                "settings: {}",
                settings_path()
                    .map(|path| path.to_string_lossy().escape_debug().to_string())
                    .unwrap_or_default()
            );
            println!(
                "endpoint: {}\nmodel: {}\ninput-language: {}\nLLM enabled: {}",
                settings.provider.endpoint,
                settings.provider.model,
                settings.language.id(),
                settings.llm_enabled
            );
            println!(
                "scope: {}\nprotocol: {}\ncloud consent: {}",
                settings.provider.scope.id(),
                settings.provider.protocol.id(),
                settings.provider.cloud_consent
            );
            if settings.provider.scope == ModelScope::Cloud {
                println!(
                    "Cloud configuration only; no remote request was made. Use probe explicitly for synthetic examples."
                );
                println!(
                    "credential environment configured: {}",
                    settings.provider.api_key_env.is_some()
                );
                return Ok(());
            }
            let status = runtime::inspect(&settings.provider).map_err(|error| error.to_string())?;
            println!("{}", status.summary());
            for model in status.models {
                println!(
                    "  {} · {} · {:.2} GiB{}",
                    model.name,
                    model.family,
                    model.size_bytes as f64 / 1_073_741_824.0,
                    if model.remote {
                        " (remote, not used)"
                    } else {
                        ""
                    }
                );
            }
            if !status.installed {
                return Err(format!(
                    "请先安装本机模型 {}；Suzaku 不会在输入期间自动下载模型",
                    settings.provider.model
                ));
            }
            Ok(())
        }
        "discover" if args.len() == 1 => {
            let resolved =
                runtime::discover(&settings.provider).map_err(|error| error.to_string())?;
            println!(
                "Discovered local model: {}\nendpoint: {}\nprotocol: {}\nMetadata only; no model loaded, downloaded, or settings changed.",
                resolved.model,
                resolved.endpoint,
                if resolved.uses_ollama_api() {
                    "ollama"
                } else {
                    "openai-compatible"
                }
            );
            Ok(())
        }
        "configure" => {
            let previous = settings;
            let (dry_run, options) = standalone_flag(&args[1..], "--dry-run")?;
            settings = configured_settings(previous.clone(), &options)?;
            if dry_run {
                print_preflight(&settings, None);
                return Ok(());
            }
            settings.save_if_unchanged(&previous)?;
            println!(
                "Saved model configuration: {} · {} · {}\nLLM enabled remains {}. Choose 重新加载模型配置 in the tray to apply.",
                settings.provider.scope.id(),
                settings.provider.model,
                settings.provider.endpoint,
                settings.llm_enabled
            );
            if settings.provider.scope == ModelScope::Cloud {
                println!(
                    "Cloud consent: {}. When enabled AND authorized, composition and recent in-session commits are sent to this endpoint. Credentials are read by the IME host from its environment, never saved here.",
                    settings.provider.cloud_consent
                );
            }
            Ok(())
        }
        "check" => {
            let (credentials, options) = standalone_flag(&args[1..], "--credentials")?;
            settings = prospective_settings(settings, &options)?;
            let status = credentials.then(|| model_credential_status(&settings.provider));
            print_preflight(&settings, status);
            if status.is_some_and(|status| !status.is_usable()) {
                return Err("Current-process credential precheck failed; no authentication or host check was performed.".into());
            }
            Ok(())
        }
        "warmup" if args.len() == 1 => {
            let started = Instant::now();
            runtime::warm_up(&settings.provider).map_err(|error| error.to_string())?;
            println!(
                "Model preloaded in {} ms; no input text was sent. Idle residency: 5 minutes.",
                started.elapsed().as_millis()
            );
            Ok(())
        }
        "probe" => probe(settings, &args[1..]),
        _ => Err(USAGE.into()),
    }
}

fn print_paths() -> Result<(), String> {
    let source = if std::env::var_os("SUZAKU_IME_CONFIG").is_some() {
        "SUZAKU_IME_CONFIG (relative values resolve against this process's current directory)"
    } else {
        default_configuration_source()
    };
    let path = settings_path().ok_or_else(|| {
        format!("No configuration path is available from {source}; empty overrides do not select a fallback.")
    })?;
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|_| "Cannot resolve this process's current directory")?
            .join(path)
    };
    println!("Current-process paths only; no configuration files read or created.");
    println!(
        "settings: {}\nsource: {source}",
        absolute.to_string_lossy().escape_debug()
    );
    println!(
        "The input-method host may use a different environment or working directory; its path was not inspected."
    );
    Ok(())
}

fn default_configuration_source() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "APPDATA"
    }
    #[cfg(target_os = "macos")]
    {
        "HOME/Library/Application Support"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if std::env::var_os("XDG_CONFIG_HOME")
            .is_some_and(|value| std::path::Path::new(&value).is_absolute())
        {
            "XDG_CONFIG_HOME"
        } else {
            "HOME/.config (unset, empty or relative XDG_CONFIG_HOME is ignored)"
        }
    }
}

/// Remove a standalone switch only in option positions, never from a value.
fn standalone_flag(args: &[String], flag: &str) -> Result<(bool, Vec<String>), String> {
    let mut found = false;
    let mut options = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] == flag {
            if found {
                return Err("开关不能重复".into());
            }
            found = true;
            index += 1;
        } else {
            options.push(args[index].clone());
            if let Some(value) = args.get(index + 1) {
                options.push(value.clone());
            }
            index += 2;
        }
    }
    Ok((found, options))
}

fn prospective_settings(current: ImeSettings, args: &[String]) -> Result<ImeSettings, String> {
    if args.is_empty() {
        Ok(current)
    } else {
        configured_settings(current, args)
    }
}

fn print_preflight(
    settings: &ImeSettings,
    credentials: Option<suzaku_map::model::ModelCredentialStatus>,
) {
    let provider = &settings.provider;
    println!("Configuration syntax valid; no network request, settings unchanged.");
    println!(
        "scope: {}\nprotocol: {}\neffective protocol: {}\nendpoint: {}\nmodel: {}\ntimeout: {} ms\ncloud consent: {}\nLLM enabled: {}",
        provider.scope.id(),
        provider.protocol.id(),
        if provider.uses_ollama_api() {
            "ollama"
        } else if provider.protocol.id() == "auto" {
            "openai-compatible"
        } else {
            provider.protocol.id()
        },
        provider.endpoint,
        provider.model,
        provider.timeout_ms,
        provider.cloud_consent,
        settings.llm_enabled
    );
    println!(
        "credential environment reference: {}",
        provider.api_key_env.as_deref().unwrap_or("none")
    );
    if let Some(status) = credentials {
        println!("current-process credential status: {}", status.id());
        println!(
            "Explicit current-process precheck only: when a credential reference is required, its environment value is checked for presence/format; otherwise no credential is read. No value is displayed. No authentication request, keyring access or host-process inspection is performed."
        );
    } else {
        println!(
            "Credential values were not read. This does not verify access, credentials, model quality, or the IME host environment."
        );
    }
}

fn probe(settings: ImeSettings, args: &[String]) -> Result<(), String> {
    let (language, options) = if args.first().is_some_and(|value| !value.starts_with('-')) {
        (Some(args[0].as_str()), &args[1..])
    } else {
        (None, args)
    };
    let (once, options) = standalone_flag(options, "--once")?;
    let languages = probe_languages(language, once)?;
    let settings = prospective_settings(settings, &options)?;
    let provider = HttpModelProvider::new(settings.provider.clone());
    if !once
        && settings.provider.scope == ModelScope::Local
        && provider.uses_ollama_api()
        && settings.provider.model != DEFAULT_MODEL
    {
        println!("Preloading configured model with an empty request…");
        runtime::warm_up(&settings.provider).map_err(|error| error.to_string())?;
    }
    println!(
        "Synthetic IME examples only; no live input is read, no engine is switched, settings unchanged. Cloud requests may incur charges."
    );
    let mut failures = 0;
    let mut probes = 0;
    for language in languages {
        let seeds: &[&str] = match language {
            BuiltinLanguage::ChineseSimplified => &["nihao"],
            BuiltinLanguage::English => &["hello", "hel", "please sen", "good m", "thank you "],
            BuiltinLanguage::Japanese => &["nihongo"],
        };
        for &seed in seeds.iter().take(if once { 1 } else { seeds.len() }) {
            let mut engine = XRTabletImeEngine::new(EngineConfig {
                default_language: language.id().into(),
                ..Default::default()
            });
            let started = Instant::now();
            engine.seed(seed);
            let local_micros = started.elapsed().as_micros();
            probes += 1;
            let request = PredictionRequest::new(
                probes,
                PredictionInput {
                    language_id: language.id().into(),
                    seed_text: seed.into(),
                    normalized_phrase: engine.candidates()[0].text.clone(),
                    context_before_cursor: String::new(),
                    confidence: 1.0,
                    degraded: false,
                },
            );
            let started = Instant::now();
            let result = provider.predict(&request, &PredictionCancellation::default());
            println!(
                "{}: local={} µs, model={} ms, literal={:?}, local-conversion={:?}",
                language.id(),
                local_micros,
                started.elapsed().as_millis(),
                seed,
                request.input.normalized_phrase
            );
            println!(
                "  offline: {:?}",
                engine
                    .candidates()
                    .iter()
                    .skip(1)
                    .map(|candidate| candidate.text.as_str())
                    .collect::<Vec<_>>()
            );
            match result {
                Ok(response) => {
                    println!(
                        "  model: {} accepted candidate(s), prediction protocol v{}",
                        response.candidates.len(),
                        response.version
                    );
                    for candidate in response.candidates {
                        println!("  {:?}", candidate.text);
                    }
                }
                Err(error) => {
                    failures += 1;
                    println!("  unavailable: {error}");
                    if let PredictionError::HttpStatus(status) = error {
                        println!(
                            "  HTTP status: {status}; response body and credentials are not displayed."
                        );
                    }
                }
            }
        }
    }
    println!(
        "{probes} synthetic probe(s), {failures} failed. Offline rows are not model acceptance. This is not a comprehensive language-quality or IME host test."
    );
    if failures > 0 {
        Err(format!(
            "{failures} synthetic probe(s) returned no valid model candidates"
        ))
    } else {
        Ok(())
    }
}

fn probe_languages(language: Option<&str>, once: bool) -> Result<Vec<BuiltinLanguage>, String> {
    match language.unwrap_or(if once { "en" } else { "all" }) {
        "all" if once => Err("--once 只能测试一种语言；请用 en、zh-Hans 或 ja".into()),
        "all" => Ok(BuiltinLanguage::ALL.to_vec()),
        value => Ok(vec![
            BuiltinLanguage::resolve(value).ok_or("不支持的测试语言")?,
        ]),
    }
}

fn configured_settings(current: ImeSettings, args: &[String]) -> Result<ImeSettings, String> {
    let mut json = current.to_json();
    if args.is_empty() {
        json["llm_endpoint"] = DEFAULT_LOCAL_ENDPOINT.into();
        json["llm_model"] = DEFAULT_MODEL.into();
        json["llm_scope"] = "local".into();
        json["llm_protocol"] = "auto".into();
        json["llm_api_key_env"] = serde_json::Value::Null;
        json["llm_cloud_consent"] = false.into();
    }
    if !args.len().is_multiple_of(2) {
        return Err("配置参数需要对应的值".into());
    }
    let mut consent = None;
    let mut base_url = None;
    let mut endpoint_supplied = false;
    let mut protocol = None;
    for pair in args.chunks_exact(2) {
        match pair[0].as_str() {
            "--model" => json["llm_model"] = pair[1].clone().into(),
            "--endpoint" => {
                endpoint_supplied = true;
                json["llm_endpoint"] = pair[1].clone().into();
            }
            "--base-url" => {
                if base_url.replace(pair[1].as_str()).is_some() {
                    return Err("--base-url 不能重复".into());
                }
            }
            "--scope" => json["llm_scope"] = pair[1].clone().into(),
            "--protocol" => {
                protocol = Some(pair[1].as_str());
                json["llm_protocol"] = pair[1].clone().into();
            }
            "--api-key-env" => {
                json["llm_api_key_env"] = if pair[1] == "none" {
                    serde_json::Value::Null
                } else {
                    pair[1].clone().into()
                }
            }
            "--cloud-consent" => {
                consent = Some(
                    pair[1]
                        .parse::<bool>()
                        .map_err(|_| "云端授权必须为 true 或 false")?,
                )
            }
            "--timeout-ms" => {
                json["llm_timeout_ms"] = pair[1]
                    .parse::<u64>()
                    .map_err(|_| "超时必须为整数毫秒")?
                    .into()
            }
            _ => {
                return Err(
                    "未知配置参数；请用 model help 查看支持的选项，不要传入明文密钥".into(),
                );
            }
        }
    }
    if let Some(base_url) = base_url {
        if endpoint_supplied {
            return Err("--base-url 与 --endpoint 不能同时使用".into());
        }
        if protocol.is_some_and(|value| {
            !matches!(
                value,
                "openai-compatible" | "chat-completions" | "chat-completions-low"
            )
        }) {
            return Err(
                "--base-url 仅支持 --protocol openai-compatible、chat-completions 或 chat-completions-low".into(),
            );
        }
        let scope = match json["llm_scope"].as_str() {
            Some("local") => ModelScope::Local,
            Some("cloud") => ModelScope::Cloud,
            _ => return Err("模型部署位置必须为 local 或 cloud".into()),
        };
        json["llm_endpoint"] = openai_compatible_endpoint_from_base_url(base_url, scope)
            .map_err(|error| error.to_string())?
            .into();
        json["llm_protocol"] = protocol.unwrap_or("openai-compatible").into();
    }
    let previous = current.to_json();
    if [
        "llm_scope",
        "llm_protocol",
        "llm_model",
        "llm_endpoint",
        "llm_api_key_env",
    ]
    .iter()
    .any(|key| json[key] != previous[key])
    {
        json["llm_cloud_consent"] = false.into();
    }
    if let Some(consent) = consent {
        json["llm_cloud_consent"] = consent.into();
    }
    ImeSettings::from_json(&json.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).into()).collect()
    }

    #[test]
    fn cloud_configuration_and_target_changes_require_explicit_consent() {
        let configured = configured_settings(
            ImeSettings::default(),
            &args(&[
                "--scope",
                "cloud",
                "--endpoint",
                "https://models.example/v1/chat/completions",
                "--model",
                "custom-family",
                "--api-key-env",
                "SUZAKU_MODEL_KEY",
            ]),
        )
        .unwrap();
        assert!(!configured.provider.cloud_consent);
        assert!(!configured.llm_enabled);
        let authorized =
            configured_settings(configured, &args(&["--cloud-consent", "true"])).unwrap();
        assert!(authorized.provider.cloud_consent);
        let changed = configured_settings(
            authorized.clone(),
            &args(&["--endpoint", "https://another.example/v1/chat/completions"]),
        )
        .unwrap();
        assert!(!changed.provider.cloud_consent);
        let tuning =
            configured_settings(authorized.clone(), &args(&["--timeout-ms", "3000"])).unwrap();
        assert!(tuning.provider.cloud_consent);
        let reset = configured_settings(authorized, &[]).unwrap();
        assert_eq!(reset.provider.scope, ModelScope::Local);
        assert_eq!(reset.provider.model, "auto");
        assert_eq!(reset.provider.api_key_env, None);
        assert!(!reset.provider.cloud_consent);
    }
    #[test]
    fn llama_configuration_preserves_language_opt_in_and_validates_before_save() {
        let current = ImeSettings {
            language: BuiltinLanguage::Japanese,
            llm_enabled: false,
            ..Default::default()
        };
        let configured = configured_settings(
            current.clone(),
            &[
                "--model".into(),
                "llama3.2:1b".into(),
                "--timeout-ms".into(),
                "2400".into(),
            ],
        )
        .unwrap();
        assert_eq!(configured.language, BuiltinLanguage::Japanese);
        assert!(!configured.llm_enabled);
        assert_eq!(configured.provider.model, "llama3.2:1b");
        assert_eq!(configured.provider.timeout_ms, 2400);
        assert!(configured_settings(current.clone(), &["--model".into()]).is_err());
        assert!(
            configured_settings(current, &["--endpoint".into(), "http://example.com".into()])
                .is_err()
        );
    }

    #[test]
    fn check_without_overrides_keeps_cloud_target_and_consent() {
        let cloud = configured_settings(
            ImeSettings::default(),
            &args(&[
                "--scope",
                "cloud",
                "--base-url",
                "https://models.example/v1/",
                "--model",
                "demo",
                "--cloud-consent",
                "true",
            ]),
        )
        .unwrap();
        let checked = prospective_settings(cloud.clone(), &[]).unwrap();
        assert_eq!(checked.to_json(), cloud.to_json());
        let changed = prospective_settings(
            cloud,
            &args(&["--base-url", "https://another.example/custom"]),
        )
        .unwrap();
        assert!(!changed.provider.cloud_consent);
        assert_eq!(
            changed.provider.endpoint,
            "https://another.example/custom/chat/completions"
        );
    }

    #[test]
    fn standalone_switches_do_not_consume_option_values_or_allow_duplicates() {
        let (once, options) =
            standalone_flag(&args(&["--once", "--model", "--once"]), "--once").unwrap();
        assert!(once);
        assert_eq!(options, args(&["--model", "--once"]));
        assert!(standalone_flag(&args(&["--once", "--once"]), "--once").is_err());
        let (dry_run, options) =
            standalone_flag(&args(&["--model", "--dry-run"]), "--dry-run").unwrap();
        assert!(!dry_run);
        assert_eq!(options, args(&["--model", "--dry-run"]));
    }

    #[test]
    fn single_probe_requires_a_single_language_and_defaults_to_english() {
        assert_eq!(
            probe_languages(None, true).unwrap(),
            vec![BuiltinLanguage::English]
        );
        assert_eq!(probe_languages(None, false).unwrap(), BuiltinLanguage::ALL);
        assert!(probe_languages(Some("all"), true).is_err());
        assert!(probe_languages(Some("unsupported"), true).is_err());
        assert_eq!(
            probe_languages(Some("ja"), true).unwrap(),
            vec![BuiltinLanguage::Japanese]
        );
    }

    #[test]
    fn base_url_is_explicit_and_errors_never_echo_unknown_arguments() {
        for protocol in ["auto", "ollama"] {
            assert!(
                configured_settings(
                    ImeSettings::default(),
                    &args(&[
                        "--base-url",
                        "http://localhost:8080/v1",
                        "--protocol",
                        protocol
                    ])
                )
                .is_err()
            );
        }
        assert!(
            configured_settings(
                ImeSettings::default(),
                &args(&[
                    "--base-url",
                    "http://localhost:8080/v1",
                    "--base-url",
                    "http://localhost:8081/v1"
                ])
            )
            .is_err()
        );
        let error = configured_settings(
            ImeSettings::default(),
            &args(&["--api-key=SYNTHETIC_DO_NOT_ECHO", "unused"]),
        )
        .unwrap_err();
        assert!(!error.contains("SYNTHETIC_DO_NOT_ECHO"));
    }

    #[test]
    fn modern_chat_profile_is_explicit_and_a_consent_relevant_target_change() {
        let original = configured_settings(
            ImeSettings::default(),
            &args(&[
                "--scope",
                "cloud",
                "--base-url",
                "https://models.example/v1",
                "--model",
                "demo",
                "--cloud-consent",
                "true",
            ]),
        )
        .unwrap();
        let modern = configured_settings(
            original.clone(),
            &args(&[
                "--base-url",
                "https://models.example/v1",
                "--protocol",
                "chat-completions",
            ]),
        )
        .unwrap();
        assert_eq!(modern.provider.protocol.id(), "chat-completions");
        assert!(!modern.provider.cloud_consent);
        assert!(!modern.provider.same_identity(&original.provider));
        assert_eq!(modern.provider.endpoint, original.provider.endpoint);
    }
}

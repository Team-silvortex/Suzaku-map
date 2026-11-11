//! Read-only API setup guidance shared by the panel and CLI.
//! Resolve names, not file contents, credentials, process environments or services.
use super::UiLanguage;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelSetupGuide {
    /// Current process's absolute spelling, not a canonical or host-confirmed path.
    pub settings_path: Option<PathBuf>,
}

impl ModelSetupGuide {
    pub fn current() -> Self {
        let settings_path = crate::ime::settings::settings_path().and_then(|path| {
            if path.is_absolute() {
                Some(path)
            } else {
                std::env::current_dir().ok().map(|cwd| cwd.join(path))
            }
        });
        Self { settings_path }
    }

    pub fn render(&self, ui: UiLanguage) -> String {
        let path = self.settings_path.as_ref().map_or_else(
            || ui.tr("Configuration path unavailable").to_owned(),
            |path| path.to_string_lossy().into_owned(),
        );
        let mut parts = vec![
            ui.tr("Model API setup").to_owned(),
            ui.tr("Configuration file in this process: {}")
                .replace("{}", &path),
        ];
        parts.extend(GUIDANCE.iter().map(|key| ui.tr(key).to_owned()));
        // No generated newlines: the report viewer deliberately escapes all
        // controls. Dynamic paths remain opaque and are escaped by that viewer.
        parts.join("  |  ")
    }

    /// Bound an untrusted dynamic path before the report formats its snapshot.
    /// Keep the original path untouched unless the display spelling exceeds the limit.
    pub fn bounded(&self, max_bytes: usize) -> (Self, bool) {
        let Some(path) = &self.settings_path else {
            return (self.clone(), false);
        };
        let display = path.to_string_lossy();
        if display.len() <= max_bytes {
            return (self.clone(), false);
        }
        let mut end = max_bytes;
        while !display.is_char_boundary(end) {
            end -= 1;
        }
        (
            Self {
                settings_path: Some(PathBuf::from(&display[..end])),
            },
            true,
        )
    }
}

const GUIDANCE: &[&str] = &[
    "This read-only guide does not change settings, read credentials, or contact a model.",
    "Command locations: .deb uses suzaku-tool; source builds use ./target/debug/suzaku_tool; tar archives use ./bin/suzaku_tool.",
    "Interactive setup: run suzaku-tool model setup in a terminal. Preview before saving; it never asks for a key value, grants cloud consent, or tests a service.",
    "1. Get the API base URL and model ID from your provider. A model picker is not API configuration.",
    "2. Run suzaku-tool model paths, then model check. Use --base-url for an API base or --endpoint for the full generation URL; never both.",
    "3. Save only the credential environment variable name with --api-key-env. Never put the key in JSON, a URL, or command arguments.",
    "4. model check --credentials checks this process only. Seahorse keys are not loaded automatically; a terminal key does not reach a running host.",
    "5. model probe en --once explicitly sends one synthetic example and may incur cloud charges. It does not read live input or save overrides.",
    "6. After saving, supply credentials to the actual host and panel processes, reload model settings in the tray, then enable LLM. Reload does not transfer environment variables.",
    "Cloud consent is separate from enabling LLM. Changing the endpoint, model, protocol, scope, or credential reference revokes old consent.",
    "Candidate deadlines stay within 20-5000 ms and offline input remains available. Passing a 5-second test does not prove the default 1200 ms experience.",
    "Full instructions: docs/model-providers.md; Linux packages: share/doc/suzaku/model-providers.md. The tray can open the input-method configuration folder.",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_interfaces_translate_owned_guidance_but_preserve_paths_and_commands() {
        let path = PathBuf::from("/private/Model On/設定/Provider.json");
        let guide = ModelSetupGuide {
            settings_path: Some(path.clone()),
        };
        for language in UiLanguage::ALL {
            let text = guide.render(language);
            assert!(text.contains(path.to_str().unwrap()));
            for command in [
                "suzaku-tool model paths",
                "suzaku-tool model setup",
                "model check",
                "--credentials",
                "--api-key-env",
                "model probe en --once",
                "docs/model-providers.md",
            ] {
                assert!(text.contains(command), "{}: {command}", language.id());
            }
            if !matches!(language.resolved(), UiLanguage::English) {
                for key in GUIDANCE {
                    assert_ne!(language.tr(key), *key, "{}: {key}", language.id());
                }
            }
        }
        assert_eq!(guide.settings_path, Some(path));
    }

    #[test]
    fn missing_path_does_not_invent_a_default_or_claim_a_host_configuration() {
        let guide = ModelSetupGuide {
            settings_path: None,
        };
        assert!(
            guide
                .render(UiLanguage::English)
                .contains("Configuration path unavailable")
        );
        assert_eq!(guide.bounded(0), (guide.clone(), false));
        assert!(
            guide
                .render(UiLanguage::English)
                .contains("this process only")
        );
    }

    #[test]
    fn dynamic_paths_are_bounded_on_utf8_boundaries_without_mutating_the_snapshot() {
        let guide = ModelSetupGuide {
            settings_path: Some(PathBuf::from("/路径/é\nOn")),
        };
        let original = guide.clone();
        for limit in 0..=24 {
            let (bounded, limited) = guide.bounded(limit);
            assert!(bounded.settings_path.unwrap().to_string_lossy().len() <= limit);
            assert_eq!(
                limited,
                original
                    .settings_path
                    .as_ref()
                    .unwrap()
                    .to_string_lossy()
                    .len()
                    > limit
            );
            assert_eq!(guide, original);
        }
    }
}

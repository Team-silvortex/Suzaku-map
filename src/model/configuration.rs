//! Explicit configuration and opt-in credential prechecks, without discovery or network IO.
//! Credential values are read only by the explicitly called precheck/its injected reader.
use super::{ModelProviderConfig, ModelScope, is_local_llm_endpoint, transport};
use crate::prediction::PredictionError;
use std::ffi::OsString;

/// Presence/format in one process, never a claim that a credential is authorized.
/// This enum deliberately cannot contain a credential value or transport error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelCredentialStatus {
    NotRequired,
    Present,
    Missing,
    Invalid,
}

impl ModelCredentialStatus {
    pub fn id(self) -> &'static str {
        match self {
            Self::NotRequired => "not-required",
            Self::Present => "present",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
        }
    }

    /// Usable for a preflight only; it does not verify service authentication,
    /// cloud consent, the input-method host's environment or model availability.
    pub fn is_usable(self) -> bool {
        matches!(self, Self::NotRequired | Self::Present)
    }
}

/// Explicitly inspect the configured credential in the current process only.
/// No network, keyring, configuration writes or host-process inspection occurs.
pub fn model_credential_status(config: &ModelProviderConfig) -> ModelCredentialStatus {
    model_credential_status_with(config, |name| std::env::var_os(name))
}

/// Injectable credential precheck. The reader is called at most once, and never
/// for local/no-auth configurations or an invalid environment-variable name.
/// Values are classified using the transport's identical authorization rule,
/// then dropped; they are never returned, formatted or included in an error.
pub fn model_credential_status_with(
    config: &ModelProviderConfig,
    read: impl FnOnce(&str) -> Option<OsString>,
) -> ModelCredentialStatus {
    if config.scope == ModelScope::Local {
        return ModelCredentialStatus::NotRequired;
    }
    let Some(name) = config.api_key_env.as_deref() else {
        return ModelCredentialStatus::NotRequired;
    };
    if !transport::valid_env_name(name) {
        return ModelCredentialStatus::Invalid;
    }
    match read(name) {
        None => ModelCredentialStatus::Missing,
        Some(value) if value.to_str().is_some_and(transport::valid_api_key) => {
            ModelCredentialStatus::Present
        }
        Some(_) => ModelCredentialStatus::Invalid,
    }
}

/// Append the OpenAI-compatible chat route to an explicitly supplied API base.
///
/// The base may include an API prefix such as `/v1` or `/gateway/v2`. No `/v1`
/// prefix is guessed: an origin-only base resolves to `/chat/completions`.
/// Existing full endpoints must continue to use the separate endpoint option.
/// This function neither changes the selected protocol nor grants cloud consent.
pub fn openai_compatible_endpoint_from_base_url(
    base: &str,
    scope: ModelScope,
) -> Result<String, PredictionError> {
    let invalid = || PredictionError::InvalidEndpoint;
    if base.is_empty()
        || base.len() > 2048
        || base.contains(['@', '?', '#', '\\'])
        || base.chars().any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(invalid());
    }
    let (scheme, tail) = base.split_once("://").ok_or_else(invalid)?;
    let (authority, path) = tail.split_once('/').unwrap_or((tail, ""));
    if authority.is_empty() {
        return Err(invalid());
    }
    // Inspect the raw path before a URL parser can normalize dot segments.
    // Preserve its spelling; only terminal slashes are removed from the base.
    let path = path.trim_end_matches('/');
    let decoded_path = unambiguous_path(path).ok_or_else(invalid)?;
    if ["chat/completions", "api/chat", "responses"]
        .iter()
        .any(|route| decoded_path == *route || decoded_path.ends_with(&format!("/{route}")))
    {
        return Err(invalid());
    }
    let prefix = if path.is_empty() {
        String::new()
    } else {
        format!("/{path}")
    };
    let endpoint = format!("{scheme}://{authority}{prefix}/chat/completions");
    let allowed = match scope {
        ModelScope::Local => is_local_llm_endpoint(&endpoint),
        ModelScope::Cloud => transport::is_cloud_endpoint(&endpoint),
    };
    if !allowed {
        return Err(invalid());
    }
    Ok(endpoint)
}

fn unambiguous_path(path: &str) -> Option<String> {
    if path.is_empty() {
        return Some(String::new());
    }
    let mut segments = Vec::new();
    for segment in path.split('/') {
        if segment.is_empty() {
            return None;
        }
        let mut decoded = Vec::with_capacity(segment.len());
        let mut bytes = segment.bytes();
        while let Some(byte) = bytes.next() {
            decoded.push(if byte == b'%' {
                let high = char::from(bytes.next()?).to_digit(16)?;
                let low = char::from(bytes.next()?).to_digit(16)?;
                (high * 16 + low) as u8
            } else {
                byte
            });
        }
        let decoded = String::from_utf8(decoded).ok()?;
        if matches!(decoded.as_str(), "." | "..")
            || decoded.chars().any(|ch| {
                ch.is_control()
                    || ch.is_whitespace()
                    || matches!(ch, '/' | '\\' | '@' | '?' | '#' | '%')
            })
        {
            return None;
        }
        segments.push(decoded);
    }
    Some(segments.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_precheck_never_reads_unneeded_or_invalid_references() {
        for config in [
            ModelProviderConfig::default(),
            ModelProviderConfig {
                scope: ModelScope::Cloud,
                api_key_env: None,
                ..Default::default()
            },
        ] {
            assert_eq!(
                model_credential_status_with(&config, |_| panic!("unneeded credential read")),
                ModelCredentialStatus::NotRequired
            );
        }
        let config = ModelProviderConfig {
            scope: ModelScope::Cloud,
            api_key_env: Some("not=a valid reference".into()),
            ..Default::default()
        };
        assert_eq!(
            model_credential_status_with(&config, |_| panic!("invalid reference read")),
            ModelCredentialStatus::Invalid
        );
    }

    #[test]
    fn credential_precheck_matches_transport_format_without_returning_values() {
        let config = ModelProviderConfig {
            scope: ModelScope::Cloud,
            api_key_env: Some("SUZAKU_SYNTHETIC_KEY".into()),
            ..Default::default()
        };
        assert_eq!(
            model_credential_status_with(&config, |_| None),
            ModelCredentialStatus::Missing
        );
        for value in [
            "SYNTHETIC_KEY_42".to_owned(),
            "!".repeat(8192),
            String::new(),
            "contains whitespace".to_owned(),
            "line\nbreak".to_owned(),
            "nul\0byte".to_owned(),
            "非ASCII".to_owned(),
            "x".repeat(8193),
        ] {
            let expected = if transport::valid_api_key(&value) {
                ModelCredentialStatus::Present
            } else {
                ModelCredentialStatus::Invalid
            };
            let status = model_credential_status_with(&config, |name| {
                assert_eq!(name, "SUZAKU_SYNTHETIC_KEY");
                Some(value.clone().into())
            });
            assert_eq!(status, expected);
            assert!(!format!("{status:?}").contains(&value) || value.is_empty());
            assert_eq!(status.is_usable(), status == ModelCredentialStatus::Present);
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_environment_credentials_are_invalid_not_missing() {
        use std::os::unix::ffi::OsStringExt;
        let config = ModelProviderConfig {
            scope: ModelScope::Cloud,
            api_key_env: Some("SUZAKU_SYNTHETIC_KEY".into()),
            ..Default::default()
        };
        assert_eq!(
            model_credential_status_with(&config, |_| Some(OsString::from_vec(vec![0xff]))),
            ModelCredentialStatus::Invalid
        );
    }

    #[test]
    fn explicit_bases_preserve_api_prefixes_without_guessing_v1() {
        for (base, expected) in [
            (
                "https://models.example",
                "https://models.example/chat/completions",
            ),
            (
                "https://models.example/",
                "https://models.example/chat/completions",
            ),
            (
                "https://models.example/v1",
                "https://models.example/v1/chat/completions",
            ),
            (
                "https://models.example/v1///",
                "https://models.example/v1/chat/completions",
            ),
            (
                "https://models.example:8443/gateway/v2/",
                "https://models.example:8443/gateway/v2/chat/completions",
            ),
            (
                "https://models.example/tenant%2Dapi/v1",
                "https://models.example/tenant%2Dapi/v1/chat/completions",
            ),
        ] {
            assert_eq!(
                openai_compatible_endpoint_from_base_url(base, ModelScope::Cloud),
                Ok(expected.into()),
                "{base}"
            );
        }
    }

    #[test]
    fn local_bases_keep_only_literal_loopback_http_without_dns_expansion() {
        for (base, expected) in [
            (
                "http://127.0.0.1:1234",
                "http://127.0.0.1:1234/chat/completions",
            ),
            (
                "http://127.0.0.1:1234/",
                "http://127.0.0.1:1234/chat/completions",
            ),
            (
                "http://localhost:8080/v1/",
                "http://localhost:8080/v1/chat/completions",
            ),
            (
                "http://[::1]:8080/gateway/v1/",
                "http://[::1]:8080/gateway/v1/chat/completions",
            ),
        ] {
            let endpoint =
                openai_compatible_endpoint_from_base_url(base, ModelScope::Local).unwrap();
            assert_eq!(endpoint, expected);
            assert!(is_local_llm_endpoint(&endpoint));
            assert!(endpoint.ends_with("/chat/completions"));
        }
        for base in [
            "http://models.example/v1",
            "http://10.0.0.1:8080/v1",
            "http://127.1:8080/v1",
            "http://0x7f000001:8080/v1",
            "http://2130706433:8080/v1",
            "https://127.0.0.1:8080/v1",
            "http://localhost.evil.example:8080/v1",
        ] {
            assert_invalid(base, ModelScope::Local);
        }
    }

    #[test]
    fn cloud_bases_require_https_and_forbid_embedded_credentials_or_url_metadata() {
        for base in [
            "http://models.example/v1",
            "ftp://models.example/v1",
            "https:///v1",
            "https://user:password@models.example/v1",
            "https://models.example/path@tenant",
            "https://models.example/v1?key=synthetic",
            "https://models.example/v1?",
            "https://models.example/v1#fragment",
            "https://models.example/v1#",
            "https://models.example:0/v1",
            "https://models.example\\v1",
            "https://models.example/v1\r\nx-extra: injected",
            " https://models.example/v1",
            "https://models.example/v1 ",
            "https://models.example/v1\u{a0}",
        ] {
            assert_invalid(base, ModelScope::Cloud);
        }
    }

    #[test]
    fn known_full_endpoints_are_not_reinterpreted_as_bases() {
        for path in [
            "/chat/completions",
            "/v1/chat/completions/",
            "/gateway/v1/chat/completions///",
            "/api/chat",
            "/gateway/api/chat/",
            "/responses",
            "/v1/responses/",
            "/v1/%72esponses",
            "/v1/chat/%63ompletions",
        ] {
            assert_invalid(&format!("https://models.example{path}"), ModelScope::Cloud);
            assert_invalid(&format!("http://127.0.0.1:8080{path}"), ModelScope::Local);
        }
    }

    #[test]
    fn raw_ambiguous_paths_are_rejected_before_url_normalization() {
        for path in [
            "/v1/../v2",
            "/v1/./v2",
            "/v1/..",
            "/v1/%2e%2e/v2",
            "/v1/.%2E/v2",
            "/v1/%2e/v2",
            "/v1//v2",
            "//v1",
            "/v1%2Fother",
            "/v1%5cother",
            "/v1%3fkey=synthetic",
            "/v1%23fragment",
            "/v1%40tenant",
            "/v1/%252e%252e",
            "/v1/%00",
            "/v1/%0a",
            "/v1/%20",
            "/v1/%c2%a0",
            "/v1/%ff",
            "/v1/%",
            "/v1/%2",
            "/v1/%zz",
        ] {
            assert_invalid(&format!("https://models.example{path}"), ModelScope::Cloud);
            assert_invalid(&format!("http://127.0.0.1:8080{path}"), ModelScope::Local);
        }
    }

    #[test]
    fn base_and_resolved_endpoint_are_both_bounded() {
        assert_invalid("", ModelScope::Cloud);
        assert_invalid(&"x".repeat(2049), ModelScope::Cloud);
        let prefix = "https://models.example/";
        let too_long_after_append = format!("{prefix}{}", "v".repeat(2048 - prefix.len()));
        assert_eq!(too_long_after_append.len(), 2048);
        assert_invalid(&too_long_after_append, ModelScope::Cloud);
        let largest = format!(
            "{prefix}{}",
            "v".repeat(2048 - prefix.len() - "/chat/completions".len())
        );
        assert_eq!(
            openai_compatible_endpoint_from_base_url(&largest, ModelScope::Cloud)
                .unwrap()
                .len(),
            2048
        );
    }

    fn assert_invalid(base: &str, scope: ModelScope) {
        assert_eq!(
            openai_compatible_endpoint_from_base_url(base, scope),
            Err(PredictionError::InvalidEndpoint),
            "{base}"
        );
    }
}

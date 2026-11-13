//! Bounded initialization diagnostics, not a retry or device-recovery policy.
use std::{
    error::Error,
    fmt::{self, Write},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use serde_json::{Value, json};

use super::PanelWindowKind;

const MAX_FIELD_CHARS: usize = 512;
const MAX_ERROR_SOURCES: usize = 8;
static NEXT_INITIALIZATION: AtomicU64 = AtomicU64::new(1);

pub(super) struct Initialization {
    id: u64,
    kind: PanelWindowKind,
    verbose: bool,
    step: u32,
    device_attempt: u32,
}

impl Initialization {
    pub(super) fn new(kind: PanelWindowKind) -> Self {
        Self::with_verbose(
            kind,
            std::env::var_os("SUZAKU_PANEL_SURFACE_DIAGNOSTICS").is_some(),
        )
    }

    fn with_verbose(kind: PanelWindowKind, verbose: bool) -> Self {
        Self {
            id: NEXT_INITIALIZATION.fetch_add(1, Ordering::Relaxed),
            kind,
            verbose,
            step: 0,
            device_attempt: 0,
        }
    }

    pub(super) fn stage(&mut self, name: &'static str) -> Stage {
        self.next_stage(name, None)
    }

    fn next_stage(
        &mut self,
        name: &'static str,
        adapter: Option<(&str, wgpu::AdapterInfo)>,
    ) -> Stage {
        self.step += 1;
        let mut header = json!({
            "pid": std::process::id(),
            "initialization": self.id,
            "window": format!("{:?}", self.kind),
            "step": self.step,
            "stage": name,
        });
        if let Some((role, info)) = adapter {
            self.device_attempt += 1;
            header["device_attempt"] = json!(self.device_attempt);
            header["role"] = json!(role);
            header["adapter"] = json!({
                "backend": format!("{:?}", info.backend),
                "device_type": format!("{:?}", info.device_type),
                "vendor": info.vendor,
                "device": info.device,
            });
            if self.verbose {
                header["adapter"]["name"] = json!(bounded_text(&info.name));
                header["adapter"]["driver"] = json!(bounded_text(&info.driver));
                header["adapter"]["driver_info"] = json!(bounded_text(&info.driver_info));
            }
        }
        let stage = Stage {
            header,
            verbose: self.verbose,
            started: Instant::now(),
        };
        // This must precede the actual request: a failed request has no surface
        // configuration from which the existing surface log could recover it.
        stage.emit("start", None);
        stage
    }

    pub(super) async fn request_device(
        &mut self,
        adapter: &wgpu::Adapter,
        descriptor: &wgpu::DeviceDescriptor<'_>,
        role: &'static str,
    ) -> Result<(wgpu::Device, wgpu::Queue), wgpu::RequestDeviceError> {
        let stage = self.next_stage("request-device", Some((role, adapter.get_info())));
        // Exactly one request; return its original typed error unchanged.
        stage.finish(adapter.request_device(descriptor).await)
    }
}

pub(super) struct Stage {
    header: Value,
    verbose: bool,
    started: Instant,
}

impl Stage {
    pub(super) fn finish<T, E: Error>(self, result: Result<T, E>) -> Result<T, E> {
        match &result {
            Ok(_) => self.emit("success", None),
            Err(error) => self.emit("failure", Some(error)),
        }
        result
    }

    fn record(&self, outcome: &str, error: Option<&dyn Error>) -> Option<String> {
        if !self.verbose && error.is_none() {
            return None;
        }
        let mut record = self.header.clone();
        record["outcome"] = json!(outcome);
        record["elapsed_ms"] =
            json!(self.started.elapsed().as_millis().min(u64::MAX as u128) as u64);
        if let Some(error) = error {
            let mut sources = Vec::new();
            let mut source = error.source();
            for _ in 0..MAX_ERROR_SOURCES {
                let Some(cause) = source else { break };
                sources.push(bounded_text(&cause));
                source = cause.source();
            }
            record["error"] = json!({
                "display": bounded_text(&error),
                "debug": bounded_text(&format_args!("{error:?}")),
                "sources": sources,
                "source_limit_reached": source.is_some(),
            });
        }
        // JSON escapes driver/error control characters. No input text, model
        // configuration, environment values or full adapter enumeration is logged.
        Some(format!("Suzaku GPU init: {record}"))
    }

    fn emit(&self, outcome: &str, error: Option<&dyn Error>) {
        if let Some(record) = self.record(outcome, error) {
            use std::io::Write as _;
            // Diagnostics are best-effort, including when stderr is closed.
            let _ = writeln!(std::io::stderr().lock(), "{record}");
        }
    }
}

fn bounded_text(value: &dyn fmt::Display) -> String {
    struct Limited {
        text: String,
        remaining: usize,
        truncated: bool,
    }
    impl Write for Limited {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            for ch in value.chars() {
                if self.remaining == 0 {
                    self.truncated = true;
                    break;
                }
                self.text.push(ch);
                self.remaining -= 1;
            }
            Ok(())
        }
    }
    let mut limited = Limited {
        text: String::new(),
        remaining: MAX_FIELD_CHARS,
        truncated: false,
    };
    let _ = write!(limited, "{value}");
    if limited.truncated {
        limited.text.push_str("…[truncated]");
    }
    limited.text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: "synthetic GPU\n\r\u{1b}not another record".into(),
            vendor: 123,
            device: 456,
            device_type: wgpu::DeviceType::Cpu,
            driver: "synthetic".into(),
            driver_info: "合成".repeat(MAX_FIELD_CHARS),
            backend: wgpu::Backend::Vulkan,
        }
    }

    fn decode(line: &str) -> Value {
        assert!(!line.contains(['\n', '\r', '\u{1b}']));
        serde_json::from_str(line.strip_prefix("Suzaku GPU init: ").unwrap()).unwrap()
    }

    #[test]
    fn stages_and_device_attempts_have_distinct_stable_identifiers() {
        let mut init = Initialization::with_verbose(PanelWindowKind::Settings, true);
        let surface = init.stage("create-surface");
        let alpha = init.next_stage("request-device", Some(("alpha-preference", adapter())));
        let selected = init.next_stage("request-device", Some(("selected-adapter", adapter())));
        assert_eq!(
            surface.header["initialization"],
            alpha.header["initialization"]
        );
        assert_eq!(
            alpha.header["initialization"],
            selected.header["initialization"]
        );
        assert_eq!(surface.header["step"], 1);
        assert_eq!(alpha.header["device_attempt"], 1);
        assert_eq!(selected.header["device_attempt"], 2);
        assert_eq!(selected.header["step"], 3);
        assert_eq!(selected.header["window"], "Settings");
        assert_ne!(
            init.id,
            Initialization::with_verbose(PanelWindowKind::Main, true).id
        );
    }

    #[test]
    fn verbose_adapter_records_escape_control_characters_and_bound_unicode() {
        let mut init = Initialization::with_verbose(PanelWindowKind::Main, true);
        let stage = init.next_stage("request-device", Some(("selected-adapter", adapter())));
        for outcome in ["start", "success"] {
            let record = decode(&stage.record(outcome, None).unwrap());
            assert_eq!(record["outcome"], outcome);
            assert_eq!(record["adapter"]["backend"], "Vulkan");
            assert_eq!(record["adapter"]["device_type"], "Cpu");
            assert_eq!(record["adapter"]["vendor"], 123);
            assert_eq!(record["adapter"]["device"], 456);
            assert_eq!(record["adapter"]["name"], adapter().name);
            let driver = record["adapter"]["driver_info"].as_str().unwrap();
            assert!(driver.ends_with("…[truncated]"));
            assert_eq!(
                driver.chars().count(),
                MAX_FIELD_CHARS + "…[truncated]".chars().count()
            );
        }
    }

    #[test]
    fn quiet_success_stays_quiet_but_failure_retains_stage_and_safe_adapter_ids() {
        let mut init = Initialization::with_verbose(PanelWindowKind::Settings, false);
        let stage = init.next_stage("request-device", Some(("alpha-preference", adapter())));
        assert!(stage.record("start", None).is_none());
        assert!(stage.record("success", None).is_none());
        let error = std::io::Error::other("synthetic device error");
        let record = decode(&stage.record("failure", Some(&error)).unwrap());
        assert_eq!(record["stage"], "request-device");
        assert_eq!(record["role"], "alpha-preference");
        assert_eq!(record["error"]["display"], "synthetic device error");
        assert!(record["adapter"].get("name").is_none());
        assert!(record["adapter"].get("driver").is_none());
    }

    #[derive(Debug)]
    struct Outer(std::io::Error);
    impl fmt::Display for Outer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("synthetic outer")
        }
    }
    impl Error for Outer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn observing_result_preserves_original_typed_error_and_source() {
        let error = Outer(std::io::Error::other("synthetic underlying"));
        let record = Initialization::with_verbose(PanelWindowKind::Main, true)
            .stage("request-adapter")
            .record("failure", Some(&error))
            .unwrap();
        assert_eq!(
            decode(&record)["error"]["sources"],
            json!(["synthetic underlying"])
        );
        let error = Box::new(error);
        let address = (&*error) as *const Outer;
        let returned = Initialization::with_verbose(PanelWindowKind::Main, false)
            .stage("request-adapter")
            .finish::<(), _>(Err(error))
            .unwrap_err();
        assert!(std::ptr::eq(address, &*returned));
        assert_eq!(
            returned.source().unwrap().to_string(),
            "synthetic underlying"
        );
        assert_eq!(
            Initialization::with_verbose(PanelWindowKind::Main, false)
                .stage("create-surface")
                .finish::<_, std::io::Error>(Ok(42))
                .unwrap(),
            42
        );
    }

    #[derive(Debug)]
    struct Cycle;
    impl fmt::Display for Cycle {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&"界\n\u{1b}".repeat(MAX_FIELD_CHARS))
        }
    }
    impl Error for Cycle {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(self)
        }
    }

    #[test]
    fn cyclic_sources_and_long_unicode_errors_produce_bounded_single_line_records() {
        let record = Initialization::with_verbose(PanelWindowKind::Settings, false)
            .stage("request-device")
            .record("failure", Some(&Cycle))
            .unwrap();
        assert!(record.len() < 32 * 1024);
        let record = decode(&record);
        assert_eq!(record["error"]["source_limit_reached"], true);
        let sources = record["error"]["sources"].as_array().unwrap();
        assert_eq!(sources.len(), MAX_ERROR_SOURCES);
        assert!(
            sources
                .iter()
                .all(|s| s.as_str().unwrap().ends_with("…[truncated]"))
        );
    }
}

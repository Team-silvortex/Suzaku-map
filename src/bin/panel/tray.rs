use super::PanelUserEvent;
use winit::event_loop::EventLoopProxy;

#[cfg(target_os = "linux")]
mod platform {
    use super::{EventLoopProxy, PanelUserEvent};
    use crate::input_method::{IBusBackend, InputMethodController, InputMethodState};
    use crate::report_view::ReportSnapshot;
    use crate::tray_data::{self, DataAction, DataReport};
    use ksni::blocking::{Handle, TrayMethods};
    use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
    use ksni::{Icon, MenuItem, ToolTip, Tray};
    use std::sync::mpsc::{self, Sender};
    use std::thread::{self, JoinHandle};
    use suzaku_map::ime::gpu::ThemePreset;
    use suzaku_map::ime::settings::PanelImeSettingsPatch;
    use suzaku_map::languages::BuiltinLanguage;
    use suzaku_map::model::{ModelProviderConfig, ModelScope, runtime as model_runtime};
    #[cfg(feature = "linux-ibus")]
    use suzaku_map::platform::linux_ibus_recovery::{Event as RecoveryEvent, Monitor};
    use suzaku_map::platform::linux_ime_control::{
        self, NativeImeStatus, NativePreferenceClearState, NativePreferenceStatus,
        PreferenceClearFailure, PreferenceClearReceipt,
    };
    use suzaku_map::prediction::PredictionError;
    use suzaku_map::ui::UiLanguage;

    const TRAY_ICON_SIZES: [i32; 4] = [22, 32, 48, 64];

    pub(crate) struct SystemTray {
        control_tx: Sender<TrayControl>,
        worker: Option<JoinHandle<()>>,
        theme: ThemePreset,
        ui_language: UiLanguage,
        has_icon: bool,
    }

    enum TrayControl {
        SetPanelVisible(bool),
        SetTheme(ThemePreset),
        SetUiLanguage(UiLanguage),
        RefreshInputMethod,
        #[cfg(feature = "linux-ibus")]
        EngineRecovery(RecoveryEvent),
        ActivateInputMethod,
        ReleaseInputMethod,
        SetLanguage(BuiltinLanguage),
        SetPredictionEnabled(bool),
        SetPanelImeSettings(PanelImeSettingsPatch),
        ReloadImeSettings,
        ClearPreferences,
        CheckModel(ModelProviderConfig),
        WarmModel(ModelProviderConfig),
        ModelReport(ModelProviderConfig, Result<String, String>),
        ManageData(DataAction),
        DataReport(DataReport),
        Quit,
        Shutdown,
    }

    #[derive(Default)]
    struct PreferenceMenuState {
        clearing: bool,
        awaiting: Option<PreferenceClearReceipt>,
        notice: Option<&'static str>,
        detail: Option<String>,
    }

    impl PreferenceMenuState {
        fn begin(&mut self) {
            self.clearing = true;
            self.awaiting = None;
            self.notice = None;
            self.detail = None;
        }

        fn finish(&mut self, result: &Result<NativeImeStatus, PreferenceClearFailure>) {
            self.clearing = false;
            match result {
                Ok(state) => {
                    self.awaiting = None;
                    self.detail = None;
                    self.notice = Some(if state.preferences.enabled {
                        "Candidate preferences cleared (memory + local file)"
                    } else {
                        "Candidate preferences cleared (memory only)"
                    });
                    self.observe(&state.preferences);
                }
                Err(failure) => {
                    self.awaiting = failure.receipt.clone();
                    self.notice = Some("Candidate preference clear unconfirmed; refresh status");
                    self.detail = Some(failure.message.clone());
                }
            }
        }

        fn observe(&mut self, preferences: &NativePreferenceStatus) {
            // A restart or someone else's later clear cannot confirm this request.
            if let (Some(receipt), Some(clear)) = (&self.awaiting, &preferences.clear)
                && receipt == &clear.receipt
            {
                match clear.state {
                    NativePreferenceClearState::Pending => {}
                    NativePreferenceClearState::Complete => {
                        self.awaiting = None;
                        self.detail = None;
                        self.notice = Some("Candidate preferences cleared (memory + local file)");
                    }
                    NativePreferenceClearState::Failed => {
                        self.awaiting = None;
                        self.notice = Some("Candidate preference clear failed");
                        self.detail = clear.error.clone().or_else(|| preferences.error.clone());
                    }
                }
            }
            // Learning can resume immediately after a successful clear. Show the
            // current cache rather than leaving an empty-cache message indefinitely.
            if (preferences.entries > 0 || preferences.pending || preferences.error.is_some())
                && matches!(
                    self.notice,
                    Some("Candidate preferences cleared (memory + local file)")
                        | Some("Candidate preferences cleared (memory only)")
                )
            {
                self.notice = None;
            }
        }

        fn clear_pending(preferences: &NativePreferenceStatus) -> bool {
            preferences
                .clear
                .as_ref()
                .is_some_and(|clear| clear.state == NativePreferenceClearState::Pending)
        }

        fn detail<'a>(
            &'a self,
            preferences: Option<&'a NativePreferenceStatus>,
        ) -> Option<&'a str> {
            self.detail
                .as_deref()
                .or_else(|| self.notice.and_then(|_| preferences?.error.as_deref()))
        }

        fn label(&self, preferences: Option<&NativePreferenceStatus>, ui: UiLanguage) -> String {
            if self.clearing {
                return "Candidate preferences: clearing…".into();
            }
            let Some(preferences) = preferences else {
                return self
                    .notice
                    .filter(|_| self.awaiting.is_some())
                    .unwrap_or("Candidate preferences unavailable")
                    .into();
            };
            if Self::clear_pending(preferences) {
                return "Candidate preferences: clearing…".into();
            }
            if let Some(notice) = self.notice {
                return notice.into();
            }
            if let Some(error) = &preferences.error {
                // message() intentionally preserves template payloads. Translate
                // this program-owned inner message before localizing the menu.
                format!("Candidate preferences: {}", ui.diagnostic(error))
            } else if preferences.pending {
                "Candidate preferences: saving locally…".into()
            } else if preferences.memory_only {
                "Candidate preferences: memory only".into()
            } else {
                "Candidate preferences: local persistence".into()
            }
        }
    }

    #[derive(Default)]
    struct InputMethodMenuState {
        system: InputMethodState,
        busy: bool,
        refresh_pending: bool,
        preferences: PreferenceMenuState,
        status_error: Option<String>,
        operation_error: Option<String>,
        native: Option<NativeImeStatus>,
        native_error: Option<String>,
        model_busy: bool,
        model_status: Option<(ModelProviderConfig, String)>,
    }

    impl InputMethodMenuState {
        fn request_refresh(&mut self, control_tx: &Sender<TrayControl>) {
            if self.busy || self.refresh_pending {
                return;
            }
            self.refresh_pending = true;
            if control_tx.send(TrayControl::RefreshInputMethod).is_err() {
                self.refresh_pending = false;
                self.operation_error = Some("Tray controller unavailable; restart Suzaku".into());
            }
        }

        fn observe_native(&mut self, state: NativeImeStatus) {
            self.preferences.observe(&state.preferences);
            self.native = Some(state);
            self.native_error = None;
        }

        fn request_control(
            &mut self,
            control_tx: &Sender<TrayControl>,
            command: TrayControl,
            send_event: impl FnOnce(PanelUserEvent),
        ) {
            if self.busy {
                return;
            }
            let activating = matches!(command, TrayControl::ActivateInputMethod);
            let clearing = matches!(command, TrayControl::ClearPreferences);
            self.busy = true;
            self.operation_error = None;
            if control_tx.send(command).is_err() {
                self.busy = false;
                self.operation_error = Some("Tray controller unavailable; restart Suzaku".into());
            } else if clearing {
                self.preferences.begin();
            } else if activating {
                // The tray callback holds the ksni service lock. Publish preparation
                // before that callback returns, so later Show/Hide actions stay newer
                // even when the worker is blocked behind a slow status refresh.
                // The worker takes the same lock before starting the system switch.
                send_event(PanelUserEvent::PrepareInputMethodActivation);
            }
        }

        fn can_activate(&self) -> bool {
            !self.busy && (!self.system.active() || self.native_error.is_some())
        }

        fn can_release(&self) -> bool {
            !self.busy && self.system.restore_engine.is_some()
        }

        fn error_detail(&self) -> Option<&str> {
            self.operation_error
                .as_deref()
                .or(self.status_error.as_deref())
                .or(self.native_error.as_deref())
        }

        fn error_details_snapshot(&self) -> Option<ReportSnapshot> {
            self.error_detail()
                .map(|error| ReportSnapshot::InputMethodError(error.to_owned()))
        }

        fn description(&self) -> String {
            if self.busy {
                return "Updating input method…".into();
            }
            if let Some(error) = self
                .operation_error
                .as_ref()
                .or(self.status_error.as_ref())
                .or(self.native_error.as_ref())
            {
                return error.clone();
            }
            if self.system.active() {
                if self.system.restore_engine.is_some() {
                    "Suzaku active; candidates appear while typing".into()
                } else {
                    "System-selected; switch back using the system input menu".into()
                }
            } else if let Some(current) = self.system.current_engine.as_deref() {
                format!("Current input method: {current}")
            } else {
                "Reading input method status…".into()
            }
        }

        fn menu_description(&self, ui: UiLanguage) -> String {
            if !self.busy
                && let Some(error) = self
                    .operation_error
                    .as_deref()
                    .or(self.status_error.as_deref())
                    .or(self.native_error.as_deref())
            {
                // This canonical outer template is localized by the final menu
                // pass; the inner diagnostic is translated at most once.
                format!("Input method error: {}", ui.diagnostic(error))
            } else {
                self.description()
            }
        }

        fn release_label(&self) -> String {
            match self.system.restore_engine.as_deref() {
                Some(_) if self.system.current_engine.is_some() && !self.system.active() => {
                    "End activation (keep current input method)".into()
                }
                Some(previous) => format!("Release and restore: {previous}"),
                None => "Release and restore previous input method".into(),
            }
        }

        fn native_unavailable_hint(&self) -> &str {
            self.native_error
                .as_deref()
                .unwrap_or("Input service not ready; activation starts it automatically")
        }

        fn menu_native_unavailable_hint(&self, ui: UiLanguage) -> String {
            match self.native_error.as_deref() {
                Some(error) => format!("Input method error: {}", ui.diagnostic(error)),
                None => self.native_unavailable_hint().into(),
            }
        }
    }

    fn input_language_label(language: BuiltinLanguage, ui: UiLanguage) -> &'static str {
        ui.tr(match language {
            BuiltinLanguage::English => "English",
            BuiltinLanguage::ChineseSimplified => "Chinese · Pinyin",
            BuiltinLanguage::Japanese => "Japanese · Romaji",
        })
    }

    // Keep metadata messages in their canonical form until the menu is drawn.
    // The CLI's RuntimeStatus::summary() remains independent of UI language, and
    // a completed check can be redrawn in a new language without another request.
    fn model_runtime_summary(status: &model_runtime::RuntimeStatus) -> String {
        if !status.installed {
            format!("Local model not found: {}", status.model)
        } else {
            let payload = format!("{} · {}", status.model, status.endpoint);
            if status.loaded == Some(true) {
                format!("Model loaded: {payload}")
            } else {
                // Unknown residency must not be presented as a loaded model.
                format!("Local model available: {payload}")
            }
        }
    }

    fn model_check_error(error: &PredictionError) -> String {
        let key = match error {
            PredictionError::Cancelled => "Model check cancelled; local input remains available",
            PredictionError::InvalidRequest => "Model check request invalid; check model settings",
            PredictionError::UnsupportedProtocol => {
                "Model protocol not supported; check model settings"
            }
            PredictionError::InvalidEndpoint => "Check the configured model and endpoint.",
            PredictionError::Unavailable => {
                "Local model or configured service unavailable. Check model settings."
            }
            PredictionError::Timeout => "Model check timed out; try again",
            PredictionError::MissingCredentials => {
                "Model authorization failed. Check the configured credentials."
            }
            PredictionError::HttpStatus(status @ (401 | 403)) => {
                return format!(
                    "Model authorization failed (HTTP {status}); check configured credentials"
                );
            }
            PredictionError::HttpStatus(status @ 404) => {
                return format!(
                    "Model or endpoint not found (HTTP {status}); check model settings"
                );
            }
            PredictionError::HttpStatus(status) => {
                return format!("Model service returned HTTP {status}; check its status");
            }
            PredictionError::InvalidResponse => {
                "Invalid model service response; local input remains available"
            }
            PredictionError::ResponseTooLarge => {
                "Model service response too large; check model settings"
            }
            PredictionError::NoCandidates => {
                "Model returned no usable result; local input remains available"
            }
            PredictionError::NoLocalModel => {
                "No local model found; start a local service or select a model"
            }
            PredictionError::CloudConsentRequired => {
                "Cloud access is not authorized. No text was sent."
            }
        };
        key.into()
    }

    fn menu_label(text: &str) -> String {
        // D-Bus menus interpret underscores as access keys. Bound error/status rows.
        let mut characters = text.chars();
        let mut label: String = characters.by_ref().take(72).collect();
        if characters.next().is_some() {
            label.push('…');
        }
        label.replace('_', "__")
    }

    impl SystemTray {
        pub(crate) fn has_icon(&self) -> bool {
            self.has_icon
        }

        pub(crate) fn request_quit(&self) -> bool {
            self.control_tx.send(TrayControl::Quit).is_ok()
        }

        pub(crate) fn set_panel_ime_settings(&self, patch: PanelImeSettingsPatch) -> bool {
            self.control_tx
                .send(TrayControl::SetPanelImeSettings(patch))
                .is_ok()
        }
        pub(crate) fn set_panel_visible(&self, visible: bool) {
            let _ = self.control_tx.send(TrayControl::SetPanelVisible(visible));
        }

        pub(crate) fn set_ui_language(&mut self, language: UiLanguage) {
            if self.ui_language != language
                && self
                    .control_tx
                    .send(TrayControl::SetUiLanguage(language))
                    .is_ok()
            {
                self.ui_language = language;
            }
        }

        pub(crate) fn set_theme(&mut self, theme: ThemePreset) {
            // The app can sync this at idle; only actual changes enqueue raster work.
            if self.theme != theme && self.control_tx.send(TrayControl::SetTheme(theme)).is_ok() {
                self.theme = theme;
            }
        }

        pub(crate) fn shutdown(mut self) {
            let _ = self.control_tx.send(TrayControl::Shutdown);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }

    struct SuzakuTray {
        proxy: EventLoopProxy<PanelUserEvent>,
        control_tx: Sender<TrayControl>,
        icons: Vec<Icon>,
        panel_visible: bool,
        ui_language: UiLanguage,
        input_method: InputMethodMenuState,
        data_busy: bool,
        data_report: Option<DataReport>,
    }

    // Service lifetime must not depend on a desktop tray extension being installed.
    // The fallback still processes lifecycle commands and sends UI acknowledgements.
    struct TrayHandle {
        live: Option<Handle<SuzakuTray>>,
        fallback: std::sync::Mutex<SuzakuTray>,
    }

    impl TrayHandle {
        fn update(&self, update: impl FnOnce(&mut SuzakuTray)) -> Option<()> {
            if let Some(live) = &self.live {
                live.update(update)
            } else {
                update(&mut self.fallback.lock().unwrap());
                Some(())
            }
        }

        fn shutdown(&self) {
            if let Some(live) = &self.live {
                live.shutdown().wait();
            }
        }
    }

    fn data_details_snapshot(report: Option<&DataReport>) -> Option<ReportSnapshot> {
        report.cloned().map(ReportSnapshot::Data)
    }

    fn report_details_item<T>(
        key: &'static str,
        snapshot: Option<ReportSnapshot>,
        send_event: impl Fn(&mut T, PanelUserEvent) + Send + 'static,
    ) -> MenuItem<T> {
        StandardItem {
            label: key.into(),
            enabled: snapshot.is_some(),
            activate: Box::new(move |target| {
                if let Some(snapshot) = &snapshot {
                    send_event(target, PanelUserEvent::ShowReportDetails(snapshot.clone()));
                }
            }),
            ..Default::default()
        }
        .into()
    }

    impl SuzakuTray {
        fn send(&self, event: PanelUserEvent) {
            let _ = self.proxy.send_event(event);
        }

        fn request_input_method(&mut self, command: TrayControl) {
            let proxy = &self.proxy;
            self.input_method
                .request_control(&self.control_tx, command, |event| {
                    let _ = proxy.send_event(event);
                });
        }

        fn request_model(&mut self, warmup: bool) {
            if self.input_method.model_busy {
                return;
            }
            if let Some(state) = &self.input_method.native {
                let config = state.settings.provider.clone();
                let command = if warmup {
                    TrayControl::WarmModel(config)
                } else {
                    TrayControl::CheckModel(config)
                };
                self.input_method.model_busy = self.control_tx.send(command).is_ok();
            }
        }

        fn request_data(&mut self, action: DataAction) {
            if !self.data_busy {
                self.data_busy = self
                    .control_tx
                    .send(TrayControl::ManageData(action))
                    .is_ok();
                if !self.data_busy {
                    self.data_report = Some(DataReport::manager_unavailable());
                }
            }
        }
    }

    impl Tray for SuzakuTray {
        const MENU_ON_ACTIVATE: bool = false;

        fn id(&self) -> String {
            "suzaku-panel".into()
        }

        fn title(&self) -> String {
            self.ui_language.tr("Suzaku Input Method").into()
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            self.panel_visible = !self.panel_visible;
            self.send(if self.panel_visible {
                PanelUserEvent::ShowPanel
            } else {
                PanelUserEvent::HidePanel
            });
        }

        fn icon_pixmap(&self) -> Vec<Icon> {
            self.icons.clone()
        }

        fn tool_tip(&self) -> ToolTip {
            ToolTip {
                icon_pixmap: self.icons.clone(),
                title: self.title(),
                description: format!(
                    "{}\n{}",
                    self.ui_language
                        .message(&self.input_method.menu_description(self.ui_language)),
                    self.ui_language
                        .tr("Right-click for input options; left-click to show or hide")
                ),
                ..Default::default()
            }
        }

        fn menu_about_to_show(&mut self) {
            // Refresh on demand, without polling or blocking the tray's D-Bus callbacks.
            self.input_method.request_refresh(&self.control_tx);
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            let preference_detail = self.input_method.preferences.detail(
                self.input_method
                    .native
                    .as_ref()
                    .map(|state| &state.preferences),
            );
            let data_summary = if self.data_busy {
                self.ui_language.tr("Managing data…").to_owned()
            } else if let Some(report) = &self.data_report {
                report.render(self.ui_language)
            } else {
                self.ui_language
                    .tr("Settings only; no input history or model weights")
                    .to_owned()
            };
            let mut menu = vec![
                CheckmarkItem {
                    label: "Activate Suzaku".into(),
                    checked: self.input_method.system.active(),
                    enabled: self.input_method.can_activate(),
                    activate: Box::new(|tray: &mut Self| {
                        if tray.input_method.can_activate() {
                            tray.request_input_method(TrayControl::ActivateInputMethod);
                        }
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: self.input_method.release_label(),
                    enabled: self.input_method.can_release(),
                    icon_name: "edit-undo".into(),
                    activate: Box::new(|tray: &mut Self| {
                        if tray.input_method.can_release() {
                            tray.request_input_method(TrayControl::ReleaseInputMethod);
                        }
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: self.input_method.menu_description(self.ui_language),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                report_details_item(
                    "Input method error details",
                    self.input_method.error_details_snapshot(),
                    |tray: &mut Self, event| tray.send(event),
                ),
                MenuItem::Separator,
                SubMenu {
                    label: self
                        .input_method
                        .native
                        .as_ref()
                        .map(|state| {
                            format!(
                                "Input language: {}",
                                input_language_label(state.settings.language, self.ui_language)
                            )
                        })
                        .unwrap_or_else(|| "Input language (host disconnected)".into()),
                    enabled: !self.input_method.busy && self.input_method.native.is_some(),
                    submenu: BuiltinLanguage::ALL
                        .into_iter()
                        .map(|language| {
                            CheckmarkItem {
                                label: input_language_label(language, self.ui_language).into(),
                                checked: self
                                    .input_method
                                    .native
                                    .as_ref()
                                    .is_some_and(|state| state.settings.language == language),
                                activate: Box::new(move |tray: &mut Self| {
                                    tray.request_input_method(TrayControl::SetLanguage(language))
                                }),
                                ..Default::default()
                            }
                            .into()
                        })
                        .collect(),
                    ..Default::default()
                }
                .into(),
                CheckmarkItem {
                    label: self
                        .input_method
                        .native
                        .as_ref()
                        .map(|state| {
                            if state.settings.provider.scope == ModelScope::Cloud {
                                "LLM suggestions (cloud, consent required)"
                            } else {
                                "LLM suggestions (local)"
                            }
                        })
                        .unwrap_or("LLM suggestions")
                        .into(),
                    enabled: !self.input_method.busy && self.input_method.native.is_some(),
                    checked: self
                        .input_method
                        .native
                        .as_ref()
                        .is_some_and(|state| state.settings.llm_enabled),
                    activate: Box::new(|tray: &mut Self| {
                        if let Some(status) = &tray.input_method.native {
                            tray.request_input_method(TrayControl::SetPredictionEnabled(
                                !status.settings.llm_enabled,
                            ));
                        }
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: self
                        .input_method
                        .native
                        .as_ref()
                        .map(|state| match state.prediction.as_str() {
                            "Pending" => "Generating; local candidates remain available".into(),
                            "Ready" => "Model candidates ready (marked AI)".into(),
                            "Unavailable" => state.prediction_error.as_deref().map_or_else(
                                || "Model unavailable; local candidates retained".into(),
                                |error| {
                                    format!(
                                        "Model diagnostic: {}",
                                        self.ui_language.diagnostic(error)
                                    )
                                },
                            ),
                            _ if state.settings.llm_enabled => {
                                if state.settings.provider.scope == ModelScope::Cloud
                                    && !state.settings.provider.cloud_consent
                                {
                                    "Cloud consent required; no input text is sent".into()
                                } else {
                                    "LLM enabled; waiting for input".into()
                                }
                            }
                            _ => "Using local candidates; no model request".into(),
                        })
                        .unwrap_or_else(|| {
                            self.input_method
                                .menu_native_unavailable_hint(self.ui_language)
                        }),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: if self.input_method.model_busy {
                        "Checking / warming model; typing remains available…".into()
                    } else if let Some(state) = &self.input_method.native {
                        self.input_method
                            .model_status
                            .as_ref()
                            .filter(|(config, _)| config == &state.settings.provider)
                            .map(|(_, status)| status.clone())
                            .unwrap_or_else(|| {
                                if state.settings.provider.scope == ModelScope::Cloud {
                                    format!(
                                        "Cloud model: {} (not checked online)",
                                        state.settings.provider.model
                                    )
                                } else if state.settings.provider.model == "auto" {
                                    "Model: local discovery, preferring LLaMA".into()
                                } else {
                                    format!(
                                        "Model: {} (not checked)",
                                        state.settings.provider.model
                                    )
                                }
                            })
                    } else {
                        "Model status unknown".into()
                    },
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Discover / check local models".into(),
                    enabled: !self.input_method.model_busy
                        && self.input_method.native.as_ref().is_some_and(|state| {
                            state.settings.provider.scope == ModelScope::Local
                        }),
                    activate: Box::new(|tray: &mut Self| tray.request_model(false)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Warm Ollama model (unload after 5 minutes idle)".into(),
                    enabled: !self.input_method.model_busy
                        && self.input_method.native.as_ref().is_some_and(|state| {
                            state.settings.provider.scope == ModelScope::Local
                                && state.settings.provider.uses_ollama_api()
                        }),
                    activate: Box::new(|tray: &mut Self| tray.request_model(true)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Reload model settings".into(),
                    enabled: !self.input_method.busy && self.input_method.native.is_some(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.request_input_method(TrayControl::ReloadImeSettings)
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: self.input_method.preferences.label(
                        self.input_method
                            .native
                            .as_ref()
                            .map(|state| &state.preferences),
                        self.ui_language,
                    ),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: preference_detail.map_or_else(String::new, |error| {
                        format!("Preference details: {}", self.ui_language.diagnostic(error))
                    }),
                    visible: preference_detail.is_some(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: self.input_method.native.as_ref().map_or_else(
                        || "Candidate preferences unavailable".into(),
                        |state| format!("Remembered preferences: {}", state.preferences.entries),
                    ),
                    visible: self.input_method.native.is_some(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Refresh candidate preferences".into(),
                    enabled: !self.input_method.busy && !self.input_method.refresh_pending,
                    activate: Box::new(|tray: &mut Self| {
                        tray.input_method.request_refresh(&tray.control_tx);
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: if self
                        .input_method
                        .native
                        .as_ref()
                        .is_some_and(|state| state.preferences.enabled)
                    {
                        "Clear candidate preferences (memory + local file)"
                    } else {
                        "Clear candidate preferences (memory only)"
                    }
                    .into(),
                    enabled: !self.input_method.busy
                        && self.input_method.native.as_ref().is_some_and(|state| {
                            !PreferenceMenuState::clear_pending(&state.preferences)
                        }),
                    activate: Box::new(|tray: &mut Self| {
                        tray.request_input_method(TrayControl::ClearPreferences)
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: if self.panel_visible {
                        "Hide panel".into()
                    } else {
                        "Show panel".into()
                    },
                    icon_name: if self.panel_visible {
                        "view-hidden".into()
                    } else {
                        "view-visible".into()
                    },
                    activate: Box::new(|tray: &mut Self| {
                        tray.panel_visible = !tray.panel_visible;
                        tray.send(if tray.panel_visible {
                            PanelUserEvent::ShowPanel
                        } else {
                            PanelUserEvent::HidePanel
                        });
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Settings".into(),
                    icon_name: "preferences-system".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.panel_visible = true;
                        tray.send(PanelUserEvent::OpenSettings);
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Reset window position".into(),
                    icon_name: "view-restore".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.panel_visible = true;
                        tray.send(PanelUserEvent::ResetPanelPosition);
                    }),
                    ..Default::default()
                }
                .into(),
                SubMenu {
                    label: "Data management".into(),
                    submenu: vec![
                        StandardItem {
                            // Filled after generic menu localization; a typed
                            // report's raw detail must never be parsed twice.
                            label: String::new(),
                            enabled: false,
                            ..Default::default()
                        }
                        .into(),
                        report_details_item(
                            "View last data operation",
                            data_details_snapshot(self.data_report.as_ref()),
                            |tray: &mut Self, event| tray.send(event),
                        ),
                        StandardItem {
                            label: "Back up settings now".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::Backup)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "Open backup folder".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::OpenBackups)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "Open input method settings folder".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::OpenIme)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "Open panel settings folder".into(),
                            enabled: !self.data_busy,
                            activate: Box::new(|tray: &mut Self| {
                                tray.request_data(DataAction::OpenPanel)
                            }),
                            ..Default::default()
                        }
                        .into(),
                        StandardItem {
                            label: "Restore after quitting: suzaku-tool data restore".into(),
                            enabled: false,
                            ..Default::default()
                        }
                        .into(),
                    ],
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: "Quit Suzaku".into(),
                    enabled: !self.input_method.busy,
                    icon_name: "application-exit".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.request_input_method(TrayControl::Quit);
                    }),
                    ..Default::default()
                }
                .into(),
            ];
            localize_menu_with_data_report(&mut menu, self.ui_language, &data_summary);
            menu
        }
    }

    // Localize the complete message before truncation, otherwise a long payload
    // can remove a template's suffix and prevent localization. Escape access-key
    // underscores exactly once here, including external prediction diagnostics.
    fn localize_menu<T>(menu: &mut [MenuItem<T>], language: UiLanguage) {
        for item in menu {
            let label = match item {
                MenuItem::Standard(item) => &mut item.label,
                MenuItem::Checkmark(item) => &mut item.label,
                MenuItem::SubMenu(item) => {
                    localize_menu(&mut item.submenu, language);
                    &mut item.label
                }
                _ => continue,
            };
            *label = menu_label(&language.message(label));
        }
    }

    fn localize_menu_with_data_report<T>(
        menu: &mut [MenuItem<T>],
        language: UiLanguage,
        data_summary: &str,
    ) {
        for item in menu {
            // This is the program-owned submenu identifier, never a report,
            // user path or translated label. Its first row is the typed report.
            if let MenuItem::SubMenu(submenu) = item
                && submenu.label == "Data management"
            {
                let (status, actions) = submenu.submenu.split_first_mut().expect("data status row");
                let MenuItem::Standard(status) = status else {
                    unreachable!("data status is a standard row")
                };
                status.label = menu_label(data_summary);
                localize_menu(actions, language);
                submenu.label = menu_label(language.tr("Data management"));
            } else {
                localize_menu(std::slice::from_mut(item), language);
            }
        }
    }

    pub(crate) fn start_system_tray(
        proxy: EventLoopProxy<PanelUserEvent>,
        panel_visible: bool,
        theme: ThemePreset,
        ui_language: UiLanguage,
    ) -> Option<SystemTray> {
        let icons: Vec<Icon> = TRAY_ICON_SIZES
            .into_iter()
            .map(|size| themed_icon(theme, size))
            .collect();
        let (control_tx, control_rx) = mpsc::channel();
        let make_tray = || SuzakuTray {
            proxy: proxy.clone(),
            control_tx: control_tx.clone(),
            icons: icons.clone(),
            panel_visible,
            ui_language,
            input_method: InputMethodMenuState {
                busy: true,
                ..Default::default()
            },
            data_busy: false,
            data_report: None,
        };
        let live = match make_tray().spawn() {
            Ok(handle) => Some(handle),
            Err(error) => {
                eprintln!("Suzaku system tray unavailable: {error}");
                None
            }
        };
        let handle = TrayHandle {
            live,
            fallback: std::sync::Mutex::new(make_tray()),
        };
        start_tray_control_worker(handle, control_tx, control_rx, theme, ui_language)
    }

    fn start_tray_control_worker(
        handle: TrayHandle,
        control_tx: Sender<TrayControl>,
        control_rx: mpsc::Receiver<TrayControl>,
        theme: ThemePreset,
        ui_language: UiLanguage,
    ) -> Option<SystemTray> {
        let model_report_tx = control_tx.clone();
        let has_icon = handle.live.is_some();
        let worker = match thread::Builder::new()
            .name("suzaku-tray-control".into())
            .spawn(move || {
                let mut input_method = InputMethodController::new(IBusBackend::default());
                let startup = input_method.start();
                #[cfg(feature = "linux-ibus")]
                let mut recovery = start_engine_recovery(&input_method, &model_report_tx);
                refresh_input_method(&handle, &mut input_method);
                let _ = handle.update(|tray| {
                    tray.input_method.busy = false;
                    tray.input_method.operation_error = startup.err();
                    if let Some(error) = &tray.input_method.operation_error {
                        tray.send(PanelUserEvent::InputMethodError(error.clone()));
                    }
                });
                while let Ok(command) = control_rx.recv() {
                    match command {
                        TrayControl::SetPanelVisible(visible) => {
                            let _ = handle.update(|tray| tray.panel_visible = visible);
                        }
                        TrayControl::SetUiLanguage(language) => {
                            let _ = handle.update(|tray| tray.ui_language = language);
                        }
                        TrayControl::SetTheme(theme) => {
                            let icons = TRAY_ICON_SIZES
                                .into_iter()
                                .map(|size| themed_icon(theme, size))
                                .collect();
                            let _ = handle.update(|tray| tray.icons = icons);
                        }
                        TrayControl::RefreshInputMethod => {
                            refresh_input_method(&handle, &mut input_method);
                            let _ = handle.update(|tray| {
                                tray.input_method.refresh_pending = false;
                            });
                        }
                        #[cfg(feature = "linux-ibus")]
                        TrayControl::EngineRecovery(event) => {
                            refresh_input_method(&handle, &mut input_method);
                            if event == RecoveryEvent::Failed {
                                let _ = handle.update(|tray| {
                                    tray.input_method.operation_error = Some(
                                        "输入法服务已恢复，但输入源恢复失败；请从系统菜单重新选择输入法".into(),
                                    );
                                });
                            }
                        }
                        TrayControl::SetLanguage(language) => {
                            publish_native_settings(
                                &handle,
                                linux_ime_control::set_language(language),
                            );
                        }
                        TrayControl::SetPredictionEnabled(enabled) => {
                            publish_native_settings(
                                &handle,
                                linux_ime_control::set_llm_enabled(enabled),
                            );
                        }
                        TrayControl::SetPanelImeSettings(patch) => {
                            publish_settings_result(
                                &handle,
                                linux_ime_control::set_panel_ime_settings(patch),
                                true,
                            );
                        }
                        TrayControl::ReloadImeSettings => {
                            publish_native_settings(&handle, linux_ime_control::reload_settings());
                        }
                        TrayControl::ClearPreferences => {
                            publish_preference_clear(
                                &handle,
                                linux_ime_control::clear_preferences_tracked(),
                            );
                        }
                        TrayControl::CheckModel(ref config)
                        | TrayControl::WarmModel(ref config) => {
                            let warmup = matches!(command, TrayControl::WarmModel(_));
                            let config = config.clone();
                            let report_tx = model_report_tx.clone();
                            let task = thread::Builder::new()
                                .name("suzaku-model-check".into())
                                .spawn(move || {
                                    let result = if warmup {
                                        model_runtime::warm_up(&config).map(|()| {
                                            format!(
                                                "Model warmed: {} (no input text sent)",
                                                config.model
                                            )
                                        })
                                    } else {
                                        model_runtime::inspect(&config)
                                            .map(|status| model_runtime_summary(&status))
                                    }
                                    .map_err(|error| model_check_error(&error));
                                    let _ =
                                        report_tx.send(TrayControl::ModelReport(config, result));
                                });
                            if task.is_err() {
                                let _ = handle.update(|tray| tray.input_method.model_busy = false);
                            }
                        }
                        TrayControl::ModelReport(config, result) => {
                            let _ = handle.update(|tray| {
                                tray.input_method.model_busy = false;
                                tray.input_method.model_status =
                                    Some((config, result.unwrap_or_else(|error| error)));
                            });
                        }
                        TrayControl::ManageData(action) => {
                            let report_tx = model_report_tx.clone();
                            if thread::Builder::new()
                                .name("suzaku-data".into())
                                .spawn(move || {
                                    let result = std::panic::catch_unwind(|| tray_data::run(action))
                                        .unwrap_or_else(|_| DataReport::worker_panicked());
                                    let _ = report_tx.send(TrayControl::DataReport(result));
                                })
                                .is_err()
                            {
                                let _ = handle.update(|tray| {
                                    tray.data_busy = false;
                                    tray.data_report = Some(DataReport::worker_start_failed());
                                });
                            }
                        }
                        TrayControl::DataReport(result) => {
                            let _ = handle.update(|tray| {
                                tray.data_busy = false;
                                tray.data_report = Some(result);
                            });
                        }
                        TrayControl::ActivateInputMethod
                        | TrayControl::ReleaseInputMethod
                        | TrayControl::Quit => {
                            let activating = matches!(command, TrayControl::ActivateInputMethod);
                            let quitting = matches!(command, TrayControl::Quit);
                            // Explicit activation/release/quit owns the switch. No recovery
                            // observer may race a user action or an intentional service stop.
                            #[cfg(feature = "linux-ibus")]
                            drop(recovery.take());
                            let _ = handle.update(|tray| tray.input_method.busy = true);
                            let result = if activating {
                                input_method.activate()
                            } else if quitting {
                                input_method.shutdown()
                            } else {
                                input_method.release()
                            };
                            let succeeded = result.is_ok();
                            #[cfg(feature = "linux-ibus")]
                            if !quitting || !succeeded {
                                recovery = start_engine_recovery(&input_method, &model_report_tx);
                            }
                            if !quitting || !succeeded {
                                refresh_input_method(&handle, &mut input_method);
                            }
                            let _ = handle.update(|tray| {
                                tray.input_method.system = input_method.state().clone();
                                tray.input_method.busy = false;
                                tray.input_method.status_error = None;
                                tray.input_method.operation_error = result.err();
                                if let Some(error) = &tray.input_method.operation_error {
                                    tray.send(PanelUserEvent::InputMethodError(error.clone()));
                                }
                                if succeeded && quitting {
                                    tray.send(PanelUserEvent::Quit);
                                }
                                // On restore failure, stay running so the user can retry.
                            });
                        }
                        TrayControl::Shutdown => {
                            #[cfg(feature = "linux-ibus")]
                            drop(recovery.take());
                            if let Err(error) = input_method.shutdown() {
                                eprintln!(
                                    "Suzaku could not safely finish input-service shutdown: {error}"
                                );
                            }
                            handle.shutdown();
                            break;
                        }
                    }
                }
            }) {
            Ok(worker) => worker,
            Err(error) => {
                eprintln!("Suzaku tray control worker unavailable: {error}");
                return None;
            }
        };
        Some(SystemTray {
            control_tx,
            worker: Some(worker),
            theme,
            ui_language,
            has_icon,
        })
    }

    #[cfg(feature = "linux-ibus")]
    fn start_engine_recovery(
        input_method: &InputMethodController<IBusBackend>,
        control_tx: &Sender<TrayControl>,
    ) -> Option<Monitor> {
        if !input_method.can_recover_engine() {
            return None;
        }
        let socket = suzaku_map::platform::linux_ime_sync::socket_path()?;
        let control_tx = control_tx.clone();
        Monitor::start(socket, move |event| {
            let _ = control_tx.send(TrayControl::EngineRecovery(event));
        })
    }

    fn refresh_input_method(
        handle: &TrayHandle,
        input_method: &mut InputMethodController<IBusBackend>,
    ) {
        let result = input_method.refresh();
        let _ = handle.update(|tray| {
            tray.input_method.system = input_method.state().clone();
            tray.input_method.status_error = result.err();
        });
        // Startup, menu actions and engine/host recovery notifications only;
        // this never polls key events or composition text.
        let native = linux_ime_control::status();
        let _ = handle.update(|tray| {
            match native {
                Ok(state) => {
                    tray.input_method.observe_native(state);
                }
                Err(error) => {
                    tray.input_method.native = None;
                    tray.input_method.native_error = Some(error);
                }
            }
            if let Some(state) = &tray.input_method.native {
                tray.send(PanelUserEvent::InputMethodSettingsChanged(
                    state.settings.clone(),
                ));
            }
        });
    }

    fn publish_native_settings(handle: &TrayHandle, result: Result<NativeImeStatus, String>) {
        publish_settings_result(handle, result, false);
    }

    fn publish_preference_clear(
        handle: &TrayHandle,
        result: Result<NativeImeStatus, PreferenceClearFailure>,
    ) {
        // One read-only reconciliation after a failed/unconfirmed request. Further
        // reads happen only on menu open or explicit refresh; never resend F here.
        let refreshed = result.as_ref().err().map(|_| linux_ime_control::status());
        let _ = handle.update(|tray| {
            tray.input_method.busy = false;
            tray.input_method.preferences.finish(&result);
            let native = match result {
                Ok(state) => Ok(state),
                Err(_) => refreshed.expect("failed clear has a status refresh"),
            };
            match native {
                Ok(state) => tray.input_method.observe_native(state),
                Err(error) => {
                    tray.input_method.native = None;
                    tray.input_method.native_error = Some(error);
                }
            }
            if let Some(state) = &tray.input_method.native {
                tray.send(PanelUserEvent::InputMethodSettingsChanged(
                    state.settings.clone(),
                ));
            }
        });
    }

    fn publish_settings_result(
        handle: &TrayHandle,
        result: Result<NativeImeStatus, String>,
        panel_write_finished: bool,
    ) {
        // A timed-out write may have succeeded. Reconcile before rolling the companion UI back.
        let (confirmed, error) = match result {
            Ok(state) => (Some(state), None),
            Err(error) => (linux_ime_control::status().ok(), Some(error)),
        };
        let _ = handle.update(|tray| {
            tray.input_method.busy = false;
            if let Some(state) = confirmed {
                tray.input_method.observe_native(state);
            } else {
                tray.input_method.native = None;
                tray.input_method.native_error = error.clone();
            }
            if panel_write_finished {
                tray.send(PanelUserEvent::PanelImeSettingsApplied(
                    tray.input_method
                        .native
                        .as_ref()
                        .map(|state| state.settings.clone()),
                ));
            } else if let Some(state) = &tray.input_method.native {
                tray.send(PanelUserEvent::InputMethodSettingsChanged(
                    state.settings.clone(),
                ));
            }
            tray.input_method.operation_error = error;
            if panel_write_finished && let Some(error) = &tray.input_method.operation_error {
                tray.send(PanelUserEvent::InputMethodError(error.clone()));
            }
        });
    }

    #[cfg(test)]
    fn suzaku_icon(size: i32) -> Icon {
        themed_icon(ThemePreset::Suzaku, size)
    }

    fn themed_icon(theme: ThemePreset, size: i32) -> Icon {
        let size = size.clamp(1, 256);
        Icon {
            width: size,
            height: size,
            data: suzaku_map::ime::gpu::theme_icon_argb(theme, size as u32),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            InputMethodMenuState, InputMethodState, TRAY_ICON_SIZES, menu_label, suzaku_icon,
        };
        use crate::input_method::SUZAKU_ENGINE;

        fn preference_status() -> super::NativeImeStatus {
            use super::*;
            NativeImeStatus {
                settings: Default::default(),
                prediction: "Disabled".into(),
                prediction_error: None,
                preferences: NativePreferenceStatus {
                    enabled: true,
                    memory_only: false,
                    pending: false,
                    error: None,
                    entries: 0,
                    clear: Some(linux_ime_control::NativePreferenceClearStatus {
                        receipt: PreferenceClearReceipt {
                            epoch: "a".repeat(32),
                            id: 7,
                        },
                        state: NativePreferenceClearState::Complete,
                        error: None,
                    }),
                },
            }
        }

        #[test]
        fn input_error_details_use_only_current_errors_in_display_priority_order() {
            use super::*;
            let raw = format!(
                "Font: Settings\n{} / raw_日_ß:{{}}",
                "long_error_".repeat(30)
            );
            let mut state = InputMethodMenuState::default();
            state.system.current_engine = Some(raw.clone());
            assert!(state.error_details_snapshot().is_none());
            state.busy = true;
            assert!(state.error_details_snapshot().is_none());
            state.native_error = Some(format!("native: {raw}"));
            state.status_error = Some(format!("status: {raw}"));
            state.operation_error = Some(format!("operation: {raw}"));
            for expected in ["operation", "status", "native"] {
                let expected = format!("{expected}: {raw}");
                assert_eq!(state.error_detail(), Some(expected.as_str()));
                assert_eq!(
                    state.error_details_snapshot(),
                    Some(ReportSnapshot::InputMethodError(expected))
                );
                if state.operation_error.is_some() {
                    state.operation_error = None;
                } else if state.status_error.is_some() {
                    state.status_error = None;
                } else {
                    state.native_error = None;
                }
            }
            assert!(state.error_details_snapshot().is_none());
            state.busy = false;
            assert!(state.error_details_snapshot().is_none());
            assert_eq!(state.system.current_engine, Some(raw));
        }

        #[test]
        fn detail_actions_localize_labels_but_send_identical_raw_snapshots_in_eight_languages() {
            use super::*;
            use crate::tray_data::DataFailure;
            let raw = format!(
                "Font: Settings\n{}\nraw_日_ß:{{}} / Permission denied (os error 13)",
                "a_very_long_user_path_".repeat(20)
            );
            for (key, snapshot) in [
                (
                    "Input method error details",
                    ReportSnapshot::InputMethodError(raw.clone()),
                ),
                (
                    "View last data operation",
                    ReportSnapshot::Data(DataReport::Failed(DataFailure::Backup(raw.clone()))),
                ),
            ] {
                for ui in UiLanguage::ALL
                    .into_iter()
                    .filter(|ui| *ui != UiLanguage::System)
                {
                    let mut menu = vec![report_details_item(
                        key,
                        Some(snapshot.clone()),
                        |events: &mut Vec<PanelUserEvent>, event| events.push(event),
                    )];
                    localize_menu(&mut menu, ui);
                    let MenuItem::Standard(item) = &menu[0] else {
                        panic!("details action")
                    };
                    assert!(item.enabled);
                    assert_eq!(item.label, menu_label(ui.tr(key)), "{ui:?} / {key}");
                    if ui != UiLanguage::English {
                        assert_ne!(ui.tr(key), key, "missing {ui:?}: {key}");
                    }
                    let mut events = Vec::new();
                    (item.activate)(&mut events);
                    assert_eq!(
                        events,
                        [PanelUserEvent::ShowReportDetails(snapshot.clone())],
                        "labels cannot translate or truncate report parameters: {ui:?} / {key}"
                    );
                }
            }
        }

        #[test]
        fn last_data_details_are_available_while_busy_without_starting_another_operation() {
            use super::*;
            use crate::tray_data::DataFailure;
            for busy in [false, true] {
                for old_report in [
                    None,
                    Some(DataReport::Failed(DataFailure::Backup(
                        "Font: Settings\nraw_日_ß:{}".into(),
                    ))),
                ] {
                    let source = old_report.clone();
                    let item = report_details_item(
                        "View last data operation",
                        data_details_snapshot(old_report.as_ref()),
                        |events: &mut Vec<PanelUserEvent>, event| events.push(event),
                    );
                    let MenuItem::Standard(item) = item else {
                        panic!("data details action")
                    };
                    assert_eq!(item.enabled, source.is_some(), "busy={busy}");
                    let mut events = Vec::new();
                    (item.activate)(&mut events);
                    assert_eq!(
                        events,
                        source
                            .clone()
                            .map(ReportSnapshot::Data)
                            .map(PanelUserEvent::ShowReportDetails)
                            .into_iter()
                            .collect::<Vec<_>>(),
                        "viewing the old report must be independent of busy={busy}"
                    );
                    assert_eq!(old_report, source);
                }
            }
        }

        #[test]
        fn bounded_data_summary_keeps_an_unmodified_full_details_snapshot() {
            use super::*;
            use crate::tray_data::DataFailure;
            let raw = format!(
                "Font: Settings\n{} / final_reason_日_ß:{{}}",
                "long_path_".repeat(40)
            );
            let report = DataReport::Failed(DataFailure::OpenFolder(raw));
            for ui in UiLanguage::ALL
                .into_iter()
                .filter(|ui| *ui != UiLanguage::System)
            {
                let summary = report.render(ui);
                assert!(summary.chars().count() > 72);
                let mut menu = vec![
                    SubMenu {
                        label: "Data management".into(),
                        submenu: vec![
                            StandardItem {
                                label: String::new(),
                                enabled: false,
                                ..Default::default()
                            }
                            .into(),
                            report_details_item(
                                "View last data operation",
                                data_details_snapshot(Some(&report)),
                                |events: &mut Vec<PanelUserEvent>, event| events.push(event),
                            ),
                        ],
                        ..Default::default()
                    }
                    .into(),
                ];
                localize_menu_with_data_report(&mut menu, ui, &summary);
                let MenuItem::SubMenu(data) = &menu[0] else {
                    panic!("data menu")
                };
                let MenuItem::Standard(status) = &data.submenu[0] else {
                    panic!("data status")
                };
                assert_eq!(status.label, menu_label(&summary));
                assert!(status.label.ends_with('…'));
                let MenuItem::Standard(details) = &data.submenu[1] else {
                    panic!("data details")
                };
                let mut events = Vec::new();
                (details.activate)(&mut events);
                assert_eq!(
                    events,
                    [PanelUserEvent::ShowReportDetails(ReportSnapshot::Data(
                        report.clone()
                    ))]
                );
            }
        }

        #[test]
        fn preference_refresh_confirms_only_the_original_clear_and_preserves_other_errors() {
            use super::*;
            for terminal in [
                NativePreferenceClearState::Complete,
                NativePreferenceClearState::Failed,
            ] {
                let mut native = preference_status();
                let receipt = native.preferences.clear.as_ref().unwrap().receipt.clone();
                let mut state = InputMethodMenuState {
                    operation_error: Some("Unrelated activation error".into()),
                    ..Default::default()
                };
                state.preferences.finish(&Err(PreferenceClearFailure {
                    message: "Timed out".into(),
                    receipt: Some(receipt.clone()),
                }));
                assert_eq!(state.preferences.detail(None), Some("Timed out"));
                for variant in 0..3 {
                    let mut unrelated = native.clone();
                    match variant {
                        0 => {
                            unrelated.preferences.clear.as_mut().unwrap().receipt.epoch =
                                "b".repeat(32)
                        }
                        1 => unrelated.preferences.clear.as_mut().unwrap().receipt.id += 1,
                        _ => unrelated.preferences.clear = None,
                    }
                    state.observe_native(unrelated);
                    assert_eq!(state.preferences.awaiting, Some(receipt.clone()));
                    assert_eq!(
                        state.preferences.notice,
                        Some("Candidate preference clear unconfirmed; refresh status")
                    );
                }
                assert_eq!(
                    state.preferences.label(None, UiLanguage::English),
                    "Candidate preference clear unconfirmed; refresh status"
                );
                native.preferences.clear.as_mut().unwrap().state =
                    NativePreferenceClearState::Pending;
                state.observe_native(native.clone());
                assert_eq!(
                    state
                        .preferences
                        .label(Some(&native.preferences), UiLanguage::English),
                    "Candidate preferences: clearing…"
                );
                assert!(state.preferences.awaiting.is_some());
                native.preferences.clear.as_mut().unwrap().state = terminal.clone();
                if terminal == NativePreferenceClearState::Failed {
                    native.preferences.clear.as_mut().unwrap().error =
                        Some("Preference storage is already in use".into());
                }
                state.observe_native(native);
                assert!(state.preferences.awaiting.is_none());
                assert_eq!(
                    state.preferences.notice,
                    Some(if terminal == NativePreferenceClearState::Complete {
                        "Candidate preferences cleared (memory + local file)"
                    } else {
                        "Candidate preference clear failed"
                    })
                );
                assert_eq!(
                    state.operation_error.as_deref(),
                    Some("Unrelated activation error")
                );
                assert_eq!(
                    state.preferences.detail(None),
                    if terminal == NativePreferenceClearState::Failed {
                        Some("Preference storage is already in use")
                    } else {
                        None
                    }
                );
                state.preferences.begin();
                assert!(state.preferences.detail(None).is_none());
            }
        }

        #[test]
        fn clear_feedback_tracks_queue_success_resumed_learning_and_unaccepted_failure() {
            use super::*;
            let (tx, rx) = mpsc::channel();
            let mut state = InputMethodMenuState::default();
            state.request_control(&tx, TrayControl::ClearPreferences, |_| {
                panic!("no activation")
            });
            assert!(matches!(rx.try_recv(), Ok(TrayControl::ClearPreferences)));
            assert_eq!(
                state.preferences.label(None, UiLanguage::English),
                "Candidate preferences: clearing…"
            );
            let mut native = preference_status();
            state.preferences.finish(&Ok(native.clone()));
            assert!(!state.preferences.clearing);
            assert_eq!(
                state.preferences.notice,
                Some("Candidate preferences cleared (memory + local file)")
            );
            native.preferences.entries = 1;
            state.observe_native(native.clone());
            assert!(state.preferences.notice.is_none());
            native.preferences.enabled = false;
            native.preferences.memory_only = true;
            native.preferences.entries = 0;
            native.preferences.clear = None;
            state.preferences.finish(&Ok(native.clone()));
            assert_eq!(
                state.preferences.notice,
                Some("Candidate preferences cleared (memory only)")
            );
            state.preferences.finish(&Err(PreferenceClearFailure {
                message: "No acknowledgement".into(),
                receipt: None,
            }));
            state.observe_native(preference_status());
            assert_eq!(
                state.preferences.notice,
                Some("Candidate preference clear unconfirmed; refresh status")
            );
        }

        #[test]
        fn preference_status_and_storage_errors_are_localized_before_menu_bounding() {
            use super::*;
            let feedback = PreferenceMenuState::default();
            let mut native = preference_status();
            native.preferences.error = Some("Invalid preference document".into());
            for ui in UiLanguage::ALL
                .into_iter()
                .filter(|ui| *ui != UiLanguage::System)
            {
                let label = feedback.label(Some(&native.preferences), ui);
                let expected = ui
                    .tr("Candidate preferences: {}")
                    .replace("{}", ui.tr("Invalid preference document"));
                assert_eq!(ui.message(&label), expected);
                let keys = [
                    "Candidate preferences: clearing…",
                    "Candidate preference clear unconfirmed; refresh status",
                    "Candidate preference clear failed",
                    "Candidate preferences cleared (memory + local file)",
                    "Candidate preferences cleared (memory only)",
                    "Clear candidate preferences (memory + local file)",
                    "Refresh candidate preferences",
                    "Remembered preferences: {}",
                ];
                let mut menu: Vec<MenuItem<()>> = keys
                    .iter()
                    .map(|key| {
                        StandardItem {
                            label: key.replace("{}", "2048"),
                            enabled: false,
                            ..Default::default()
                        }
                        .into()
                    })
                    .collect();
                localize_menu(&mut menu, ui);
                for (item, key) in menu.iter().zip(keys) {
                    let MenuItem::Standard(item) = item else {
                        panic!("standard row")
                    };
                    assert_eq!(item.label, menu_label(&ui.tr(key).replace("{}", "2048")));
                    if ui != UiLanguage::English {
                        assert_ne!(ui.tr(key), key);
                    }
                }
            }
            native.preferences.error = None;
            native.preferences.pending = true;
            assert_eq!(
                feedback.label(Some(&native.preferences), UiLanguage::English),
                "Candidate preferences: saving locally…"
            );
            native.preferences.clear.as_mut().unwrap().state = NativePreferenceClearState::Pending;
            assert_eq!(
                feedback.label(Some(&native.preferences), UiLanguage::English),
                "Candidate preferences: clearing…"
            );
        }

        #[test]
        fn input_errors_use_diagnostic_boundaries_without_reinterpreting_external_labels() {
            use super::*;
            for ui in UiLanguage::ALL
                .into_iter()
                .filter(|ui| *ui != UiLanguage::System)
            {
                for raw in [
                    "无法读取 IBus 状态，请重试",
                    "Cannot read IBus status; retry",
                    "Font: Settings",
                    "Settings",
                    "raw_日_ß:{}\nFont: custom settings",
                ] {
                    for which in 0..3 {
                        let mut state = InputMethodMenuState::default();
                        match which {
                            0 => state.operation_error = Some(raw.into()),
                            1 => state.status_error = Some(raw.into()),
                            _ => state.native_error = Some(raw.into()),
                        }
                        let label = state.menu_description(ui);
                        let expected = ui
                            .tr("Input method error: {}")
                            .replace("{}", &ui.diagnostic(raw));
                        assert_eq!(ui.message(&label), expected);
                        assert_eq!(state.description(), raw, "stored diagnosis stays unchanged");
                        let mut menu: Vec<MenuItem<()>> = vec![
                            StandardItem {
                                label,
                                enabled: false,
                                ..Default::default()
                            }
                            .into(),
                        ];
                        localize_menu(&mut menu, ui);
                        let MenuItem::Standard(item) = &menu[0] else {
                            panic!("error row")
                        };
                        assert_eq!(item.label, menu_label(&expected));
                    }
                }
            }
        }

        #[test]
        fn typed_data_reports_are_localized_once_without_changing_nested_parameters() {
            use super::*;
            use crate::tray_data::DataFailure;
            for raw in [
                "Font: custom settings",
                "Settings",
                "Could not open data folder: Font: Settings",
                "raw_日_ß:{}\nFont: custom settings",
            ] {
                let report = DataReport::Failed(DataFailure::OpenFolder(raw.into()));
                let before = report.clone();
                for ui in UiLanguage::ALL
                    .into_iter()
                    .filter(|ui| *ui != UiLanguage::System)
                {
                    let label = report.render(ui);
                    let mut menu: Vec<MenuItem<()>> = vec![
                        StandardItem {
                            label: "Settings".into(),
                            ..Default::default()
                        }
                        .into(),
                        SubMenu {
                            label: "Data management".into(),
                            submenu: vec![
                                StandardItem {
                                    label: String::new(),
                                    enabled: false,
                                    ..Default::default()
                                }
                                .into(),
                                StandardItem {
                                    label: "Back up settings now".into(),
                                    ..Default::default()
                                }
                                .into(),
                            ],
                            ..Default::default()
                        }
                        .into(),
                    ];
                    localize_menu_with_data_report(&mut menu, ui, &label);
                    let MenuItem::Standard(settings) = &menu[0] else {
                        panic!("settings")
                    };
                    assert_eq!(settings.label, menu_label(ui.tr("Settings")));
                    let MenuItem::SubMenu(data) = &menu[1] else {
                        panic!("data menu")
                    };
                    assert_eq!(data.label, menu_label(ui.tr("Data management")));
                    let MenuItem::Standard(summary) = &data.submenu[0] else {
                        panic!("summary")
                    };
                    assert_eq!(summary.label, menu_label(&label));
                    let MenuItem::Standard(backup) = &data.submenu[1] else {
                        panic!("backup")
                    };
                    assert_eq!(backup.label, menu_label(ui.tr("Back up settings now")));
                    assert_eq!(
                        report, before,
                        "rendering must not repeat or mutate the data operation"
                    );
                }
            }
        }

        #[test]
        fn refresh_is_coalesced_without_delaying_activation_preparation() {
            use super::*;
            let (tx, rx) = mpsc::channel();
            let mut state = InputMethodMenuState::default();
            state.request_refresh(&tx);
            state.request_refresh(&tx);
            assert!(state.refresh_pending);
            assert!(!state.busy);
            let mut prepared = false;
            state.request_control(&tx, TrayControl::ActivateInputMethod, |_| prepared = true);
            assert!(prepared);
            state.request_refresh(&tx);
            assert!(matches!(rx.try_recv(), Ok(TrayControl::RefreshInputMethod)));
            assert!(matches!(
                rx.try_recv(),
                Ok(TrayControl::ActivateInputMethod)
            ));
            assert!(rx.try_recv().is_err());
            state.refresh_pending = false; // Worker finished only its queued read.
            assert!(state.busy);
            state.busy = false;
            drop(rx);
            state.request_refresh(&tx);
            assert!(!state.refresh_pending);
            assert!(!state.busy);
            assert_eq!(
                state.operation_error.as_deref(),
                Some("Tray controller unavailable; restart Suzaku")
            );
        }

        #[test]
        fn activation_prepares_before_delayed_worker_and_newer_visibility_actions() {
            use super::*;
            let (control_tx, control_rx) = mpsc::channel();
            let mut state = InputMethodMenuState {
                operation_error: Some("Previous activation failed".into()),
                ..Default::default()
            };
            let mut events = Vec::new();

            // AboutToShow can queue a slow refresh without marking activation busy.
            // Leave both requests unread to verify preparation never waits for it.
            control_tx.send(TrayControl::RefreshInputMethod).unwrap();
            state.request_control(&control_tx, TrayControl::ActivateInputMethod, |event| {
                events.push(event);
            });
            assert!(state.busy);
            assert!(state.operation_error.is_none());
            assert!(matches!(
                events.as_slice(),
                [PanelUserEvent::PrepareInputMethodActivation]
            ));

            // Later tray callbacks must remain later in the same UI event stream.
            events.push(PanelUserEvent::ShowPanel);
            events.push(PanelUserEvent::HidePanel);
            assert!(matches!(
                control_rx.try_recv(),
                Ok(TrayControl::RefreshInputMethod)
            ));
            assert!(matches!(
                control_rx.try_recv(),
                Ok(TrayControl::ActivateInputMethod)
            ));
            assert!(control_rx.try_recv().is_err());
            assert!(matches!(
                events.as_slice(),
                [
                    PanelUserEvent::PrepareInputMethodActivation,
                    PanelUserEvent::ShowPanel,
                    PanelUserEvent::HidePanel
                ]
            ));
        }

        #[test]
        fn busy_input_method_rejects_activation_without_preparing_or_clearing_error() {
            use super::*;
            let (control_tx, control_rx) = mpsc::channel();
            let mut state = InputMethodMenuState {
                busy: true,
                operation_error: Some("Existing operation error".into()),
                ..Default::default()
            };
            state.request_control(&control_tx, TrayControl::ActivateInputMethod, |_| {
                panic!("A rejected activation must not hide the panel");
            });
            assert!(state.busy);
            assert_eq!(
                state.operation_error.as_deref(),
                Some("Existing operation error")
            );
            assert!(control_rx.try_recv().is_err());
        }

        #[test]
        fn disconnected_input_method_worker_does_not_prepare_and_keeps_retry_available() {
            use super::*;
            let (control_tx, control_rx) = mpsc::channel();
            drop(control_rx);
            let mut state = InputMethodMenuState::default();
            state.request_control(&control_tx, TrayControl::ActivateInputMethod, |_| {
                panic!("An activation that was not queued must not hide the panel");
            });
            assert!(!state.busy);
            assert!(state.can_activate());
            assert_eq!(
                state.operation_error.as_deref(),
                Some("Tray controller unavailable; restart Suzaku")
            );
        }

        #[test]
        fn non_activation_requests_never_prepare_panel_visibility() {
            use super::*;
            for command in [
                TrayControl::ReleaseInputMethod,
                TrayControl::SetLanguage(BuiltinLanguage::English),
                TrayControl::SetPredictionEnabled(false),
                TrayControl::ReloadImeSettings,
                TrayControl::ClearPreferences,
                TrayControl::Quit,
            ] {
                let (control_tx, control_rx) = mpsc::channel();
                let expected = std::mem::discriminant(&command);
                let mut state = InputMethodMenuState::default();
                state.request_control(&control_tx, command, |_| {
                    panic!("Only activation may prepare panel visibility");
                });
                assert!(state.busy);
                assert!(state.operation_error.is_none());
                assert_eq!(
                    std::mem::discriminant(&control_rx.try_recv().unwrap()),
                    expected
                );
                assert!(control_rx.try_recv().is_err());
            }
        }

        #[test]
        fn language_updates_are_deduplicated_and_nested_menus_keep_actions_and_ids() {
            use super::*;
            let (control_tx, control_rx) = std::sync::mpsc::channel();
            let mut tray = SystemTray {
                control_tx,
                worker: None,
                theme: ThemePreset::Suzaku,
                ui_language: UiLanguage::English,
                has_icon: true,
            };
            tray.set_ui_language(UiLanguage::English);
            assert!(control_rx.try_recv().is_err());
            for language in UiLanguage::ALL {
                tray.set_ui_language(language);
                assert!(
                    matches!(control_rx.try_recv(), Ok(TrayControl::SetUiLanguage(actual)) if actual == language)
                );
                tray.set_ui_language(language);
                assert!(control_rx.try_recv().is_err());
                let mut menu: Vec<MenuItem<()>> = vec![
                    SubMenu {
                        label: "Settings".into(),
                        submenu: vec![
                            StandardItem {
                                label: "Current input method: some_ime".into(),
                                enabled: false,
                                ..Default::default()
                            }
                            .into(),
                        ],
                        ..Default::default()
                    }
                    .into(),
                ];
                localize_menu(&mut menu, language);
                let MenuItem::SubMenu(item) = &menu[0] else {
                    panic!("submenu retained")
                };
                assert_eq!(item.label, language.tr("Settings"));
                let MenuItem::Standard(child) = &item.submenu[0] else {
                    panic!("item retained")
                };
                assert!(!child.enabled);
                assert!(child.label.ends_with("some__ime"));
            }
        }

        #[test]
        fn unchanged_theme_does_not_enqueue_tray_raster_work() {
            use super::{SystemTray, ThemePreset, TrayControl};
            let (control_tx, control_rx) = std::sync::mpsc::channel();
            let mut tray = SystemTray {
                control_tx,
                worker: None,
                theme: ThemePreset::Suzaku,
                ui_language: Default::default(),
                has_icon: true,
            };
            tray.set_theme(ThemePreset::Suzaku);
            assert!(control_rx.try_recv().is_err());
            for theme in [
                ThemePreset::Baihu,
                ThemePreset::Qinglong,
                ThemePreset::Xuanwu,
                ThemePreset::Suzaku,
            ] {
                tray.set_theme(theme);
                assert!(
                    matches!(control_rx.try_recv(), Ok(TrayControl::SetTheme(actual)) if actual == theme)
                );
                for _ in 0..20 {
                    tray.set_theme(theme);
                }
                assert!(control_rx.try_recv().is_err());
            }
        }

        #[test]
        fn input_method_menu_tracks_owned_activation_and_manual_switches() {
            let mut state = InputMethodMenuState::default();
            assert!(state.can_activate());
            assert!(!state.can_release());
            state.system = InputMethodState {
                current_engine: Some(SUZAKU_ENGINE.into()),
                restore_engine: Some("rime".into()),
            };
            assert!(!state.can_activate());
            assert!(state.can_release());
            assert!(
                state
                    .description()
                    .contains("candidates appear while typing")
            );
            assert_eq!(state.release_label(), "Release and restore: rime");
            state.system.current_engine = Some("mozc-jp".into());
            assert!(state.can_activate());
            assert!(state.can_release());
            assert!(state.release_label().contains("keep current input method"));
        }

        #[test]
        fn pending_input_method_action_disables_both_actions() {
            let state = InputMethodMenuState {
                system: InputMethodState {
                    current_engine: Some("rime".into()),
                    restore_engine: Some("rime".into()),
                },
                busy: true,
                ..Default::default()
            };
            assert!(!state.can_activate());
            assert!(!state.can_release());
            assert!(state.description().contains("Updating input method"));
        }

        #[test]
        fn restore_errors_are_visible_and_leave_the_release_action_enabled() {
            let state = InputMethodMenuState {
                system: InputMethodState {
                    current_engine: None,
                    restore_engine: Some("rime".into()),
                },
                operation_error: Some("恢复失败，请重试".into()),
                ..Default::default()
            };
            assert!(state.can_release());
            assert_eq!(state.description(), "恢复失败，请重试");
        }

        #[test]
        fn disconnected_host_reports_connection_failure_not_a_required_upgrade() {
            let mut state = InputMethodMenuState {
                native_error: Some("Suzaku 输入法服务未运行或连接已断开".into()),
                ..Default::default()
            };
            assert_eq!(state.description(), state.native_unavailable_hint());
            assert!(!state.native_unavailable_hint().contains("新版"));
            state.system.current_engine = Some(SUZAKU_ENGINE.into());
            assert!(
                state.can_activate(),
                "a disconnected active engine must be restartable"
            );
            state.busy = true;
            assert!(!state.can_activate());
            assert!(
                InputMethodMenuState::default()
                    .native_unavailable_hint()
                    .contains("starts it automatically")
            );
        }

        #[test]
        fn external_activation_explains_why_restore_is_unavailable() {
            let state = InputMethodMenuState {
                system: InputMethodState {
                    current_engine: Some(SUZAKU_ENGINE.into()),
                    restore_engine: None,
                },
                ..Default::default()
            };
            assert!(!state.can_release());
            assert!(state.description().contains("System-selected"));
        }

        #[test]
        fn input_method_menu_labels_preserve_underscores_and_bound_long_errors() {
            assert_eq!(menu_label("some_ime"), "some__ime");
            let label = menu_label(&"错".repeat(100));
            assert_eq!(label.chars().count(), 73);
            assert!(label.ends_with('…'));
        }

        #[test]
        fn model_check_status_localizes_all_states_without_translating_metadata() {
            use super::*;
            let model = "Settings";
            let endpoint = "http://x_y/Font";
            for (installed, loaded, key) in [
                (false, None, "Local model not found: {}"),
                (false, Some(true), "Local model not found: {}"),
                (true, None, "Local model available: {}"),
                (true, Some(false), "Local model available: {}"),
                (true, Some(true), "Model loaded: {}"),
            ] {
                let status = model_runtime::RuntimeStatus {
                    endpoint: endpoint.into(),
                    model: model.into(),
                    installed,
                    loaded,
                    models: vec![],
                };
                let before = status.clone();
                let raw = model_runtime_summary(&status);
                let payload = if installed {
                    format!("{model} · {endpoint}")
                } else {
                    model.into()
                };
                assert_eq!(raw, key.replace("{}", &payload));
                for language in UiLanguage::ALL
                    .into_iter()
                    .filter(|language| *language != UiLanguage::System)
                {
                    assert_eq!(
                        language.message(&raw),
                        language.tr(key).replace("{}", &payload),
                        "{language:?} {key}"
                    );
                    if language != UiLanguage::English {
                        assert_ne!(language.tr(key), key, "missing translation: {key}");
                    }
                }
                assert_eq!(status, before, "presentation must not change metadata");
                assert_eq!(
                    status.summary(),
                    if !installed {
                        format!("未找到本机模型：{model}")
                    } else if loaded == Some(true) {
                        format!("模型已加载：{model} · {endpoint}")
                    } else {
                        format!("本机模型可用：{model} · {endpoint}")
                    },
                    "CLI summary stays independent of the UI presentation"
                );
            }
        }

        #[test]
        fn model_check_failures_are_typed_localizable_and_retain_http_status() {
            use super::*;
            for (error, key, payload) in [
                (
                    PredictionError::Cancelled,
                    "Model check cancelled; local input remains available",
                    "",
                ),
                (
                    PredictionError::InvalidRequest,
                    "Model check request invalid; check model settings",
                    "",
                ),
                (
                    PredictionError::UnsupportedProtocol,
                    "Model protocol not supported; check model settings",
                    "",
                ),
                (
                    PredictionError::InvalidEndpoint,
                    "Check the configured model and endpoint.",
                    "",
                ),
                (
                    PredictionError::Unavailable,
                    "Local model or configured service unavailable. Check model settings.",
                    "",
                ),
                (
                    PredictionError::Timeout,
                    "Model check timed out; try again",
                    "",
                ),
                (
                    PredictionError::HttpStatus(401),
                    "Model authorization failed (HTTP {}); check configured credentials",
                    "401",
                ),
                (
                    PredictionError::HttpStatus(403),
                    "Model authorization failed (HTTP {}); check configured credentials",
                    "403",
                ),
                (
                    PredictionError::HttpStatus(404),
                    "Model or endpoint not found (HTTP {}); check model settings",
                    "404",
                ),
                (
                    PredictionError::HttpStatus(429),
                    "Model service returned HTTP {}; check its status",
                    "429",
                ),
                (
                    PredictionError::HttpStatus(503),
                    "Model service returned HTTP {}; check its status",
                    "503",
                ),
                (
                    PredictionError::InvalidResponse,
                    "Invalid model service response; local input remains available",
                    "",
                ),
                (
                    PredictionError::ResponseTooLarge,
                    "Model service response too large; check model settings",
                    "",
                ),
                (
                    PredictionError::NoCandidates,
                    "Model returned no usable result; local input remains available",
                    "",
                ),
                (
                    PredictionError::NoLocalModel,
                    "No local model found; start a local service or select a model",
                    "",
                ),
                (
                    PredictionError::CloudConsentRequired,
                    "Cloud access is not authorized. No text was sent.",
                    "",
                ),
                (
                    PredictionError::MissingCredentials,
                    "Model authorization failed. Check the configured credentials.",
                    "",
                ),
            ] {
                let before = error.clone();
                let raw = model_check_error(&error);
                assert_eq!(raw, key.replace("{}", payload));
                for language in UiLanguage::ALL
                    .into_iter()
                    .filter(|language| *language != UiLanguage::System)
                {
                    assert_eq!(
                        language.message(&raw),
                        language.tr(key).replace("{}", payload),
                        "{language:?} {error:?}"
                    );
                    if language != UiLanguage::English {
                        assert_ne!(language.tr(key), key, "missing translation: {key}");
                    }
                }
                assert_eq!(error, before);
            }
        }

        #[test]
        fn completed_model_check_redraws_in_new_language_and_bounds_metadata_once() {
            use super::*;
            let status = model_runtime::RuntimeStatus {
                endpoint: "http://127.0.0.1:11434/api/chat".into(),
                model: "Font: Settings_model_字体_".repeat(12),
                installed: true,
                loaded: Some(true),
                models: vec![],
            };
            let raw = model_runtime_summary(&status);
            let config = ModelProviderConfig::default();
            let state = InputMethodMenuState {
                model_status: Some((config.clone(), raw.clone())),
                ..Default::default()
            };
            let (control_tx, control_rx) = mpsc::channel();
            let mut tray = SystemTray {
                control_tx,
                worker: None,
                theme: ThemePreset::Suzaku,
                ui_language: UiLanguage::English,
                has_icon: true,
            };
            for language in [
                UiLanguage::Japanese,
                UiLanguage::German,
                UiLanguage::English,
            ] {
                tray.set_ui_language(language);
                assert!(matches!(
                    control_rx.try_recv(),
                    Ok(TrayControl::SetUiLanguage(actual)) if actual == language
                ));
                assert!(control_rx.try_recv().is_err(), "no repeat model request");
                let (stored_config, stored) = state.model_status.as_ref().unwrap();
                assert_eq!(stored_config, &config);
                assert_eq!(
                    stored, &raw,
                    "a language switch must not rewrite the report"
                );
                let payload = format!("{} · {}", status.model, status.endpoint);
                let expected = menu_label(&language.tr("Model loaded: {}").replace("{}", &payload));
                let mut menu: Vec<MenuItem<()>> = vec![
                    StandardItem {
                        label: stored.clone(),
                        enabled: false,
                        ..Default::default()
                    }
                    .into(),
                ];
                localize_menu(&mut menu, language);
                let MenuItem::Standard(item) = &menu[0] else {
                    panic!("model report retained")
                };
                assert_eq!(item.label, expected);
                assert!(!item.enabled);
            }
        }

        #[test]
        fn final_menu_labels_localize_before_bounding_and_escape_external_identifiers_once() {
            use super::*;
            for language in UiLanguage::ALL {
                for (key, payload) in [
                    ("Model: {} (not checked)", "long_model_name_".repeat(12)),
                    (
                        "Backup saved: {} (see backup folder)",
                        "备份_file_".repeat(12),
                    ),
                    ("Current input method: {}", "some_ime".into()),
                    ("{}", "external_error_".repeat(12)),
                ] {
                    let raw = key.replace("{}", &payload);
                    let expected = menu_label(&language.message(&raw));
                    let mut menu: Vec<MenuItem<()>> = vec![
                        StandardItem {
                            label: raw,
                            enabled: false,
                            ..Default::default()
                        }
                        .into(),
                    ];
                    localize_menu(&mut menu, language);
                    let MenuItem::Standard(item) = &menu[0] else {
                        panic!("status retained")
                    };
                    assert_eq!(item.label, expected, "{language:?} {key}");
                    assert!(!item.enabled);
                }
            }
        }

        #[test]
        fn tray_icon_is_argb_at_each_advertised_size() {
            for size in TRAY_ICON_SIZES {
                let icon = suzaku_icon(size);
                assert_eq!(icon.width, size);
                assert_eq!(icon.height, size);
                assert_eq!(icon.data.len(), size as usize * size as usize * 4);
                assert!(icon.data.chunks_exact(4).any(|pixel| pixel[0] > 0));
                assert_eq!(icon.data[0], 0);
            }
        }

        #[test]
        fn tray_icon_contains_suzaku_red_and_gold() {
            let icon = suzaku_icon(64);
            let pixels = icon.data.chunks_exact(4).collect::<Vec<_>>();
            let red_pixels = pixels
                .iter()
                .filter(|pixel| pixel[0] > 0 && pixel[1] > 170 && pixel[2] < 90)
                .count();
            let gold_pixels = pixels
                .iter()
                .filter(|pixel| pixel[0] > 0 && pixel[1] > 240 && pixel[2] > 170)
                .count();

            assert!(red_pixels > 500);
            assert!(gold_pixels > 100);
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::{EventLoopProxy, PanelUserEvent};

    pub(crate) struct SystemTray;

    impl SystemTray {
        pub(crate) fn has_icon(&self) -> bool {
            false
        }
        pub(crate) fn request_quit(&self) -> bool {
            false
        }
        pub(crate) fn set_panel_visible(&self, _visible: bool) {}
        pub(crate) fn set_theme(&mut self, _theme: suzaku_map::ime::gpu::ThemePreset) {}
        pub(crate) fn set_ui_language(&mut self, _language: suzaku_map::ui::UiLanguage) {}
        pub(crate) fn set_panel_ime_settings(
            &self,
            _patch: suzaku_map::ime::settings::PanelImeSettingsPatch,
        ) -> bool {
            false
        }

        pub(crate) fn shutdown(self) {}
    }

    pub(crate) fn start_system_tray(
        _proxy: EventLoopProxy<PanelUserEvent>,
        _panel_visible: bool,
        _theme: suzaku_map::ime::gpu::ThemePreset,
        _language: suzaku_map::ui::UiLanguage,
    ) -> Option<SystemTray> {
        None
    }
}

pub(super) use platform::{SystemTray, start_system_tray};

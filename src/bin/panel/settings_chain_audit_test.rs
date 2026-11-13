//! F42/F44 regressions. Private windows/configuration only, never the desktop.
use super::*;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};
use suzaku_map::{ime::settings::ImeSettings, platform::settings_host::display_settings_path};
use winit::platform::x11::EventLoopBuilderExtX11;

fn run_probe(check: fn(&mut PanelApp, &ActiveEventLoop) -> Vec<String>) {
    assert_eq!(
        std::env::var("SUZAKU_SETTINGS_CHAIN_AUDIT").as_deref(),
        Ok("1")
    );
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    for key in [
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_RUNTIME_DIR",
        "SUZAKU_IME_CONFIG",
    ] {
        assert!(PathBuf::from(std::env::var_os(key).unwrap()).starts_with(std::env::temp_dir()));
    }
    assert_ne!(
        std::env::var("DISPLAY").unwrap().split('.').next().unwrap(),
        ":0"
    );
    ImeSettings::default().save().unwrap(); // Disabled LLM; no model is contacted.
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
    struct Probe {
        proxy: EventLoopProxy<PanelUserEvent>,
        check: fn(&mut PanelApp, &ActiveEventLoop) -> Vec<String>,
        findings: Option<Vec<String>>,
    }
    impl ApplicationHandler<PanelUserEvent> for Probe {
        fn resumed(&mut self, events: &ActiveEventLoop) {
            let window = Arc::new(events.create_window(panel_window_attributes()).unwrap());
            let panel = pollster::block_on(PanelState::new(window)).unwrap();
            assert!(!panel.chrome.llm_enabled);
            let mut app = PanelApp::new(self.proxy.clone(), None);
            // Do not run PanelApp::resumed: no tray/service/desktop workers are started.
            app.panel = Some(panel);
            app.open_settings(events);
            self.findings = Some((self.check)(&mut app, events));
            events.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut probe = Probe {
        proxy: events.create_proxy(),
        check,
        findings: None,
    };
    events.run_app(&mut probe).unwrap();
    let findings = probe.findings.expect("probe completed");
    for finding in &findings {
        eprintln!("AUDIT: {finding}");
    }
    assert!(
        findings.is_empty(),
        "{} settings-chain failures",
        findings.len()
    );
}

fn click_setting(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    target: InteractionKind,
    search: &str,
) {
    let settings = app.settings.as_mut().unwrap();
    settings.chrome.settings_search_query = search.into();
    settings.chrome.settings_scroll_offset = 0.0;
    settings.last_interaction_action = None;
    crate::native_sync::present_test_frame(settings);
    let scene = settings
        .last_scene
        .as_ref()
        .expect("presented settings fixture");
    let rect = scene
        .interactive_targets
        .iter()
        .find(|item| item.kind == target)
        .expect("setting is reachable")
        .rect;
    let point = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
    assert_eq!(scene.hit_interaction(point.0, point.1), Some(target));
    settings.cursor_position = Some(point);
    settings.begin_primary_press(false);
    settings.complete_primary_release(false);
    let id = settings.window.id();
    app.window_event(events, id, WindowEvent::RedrawRequested);
}

#[test]
#[ignore = "requires private Xvfb/configuration; run by test-settings-chain-audit.sh in Linux CI"]
fn audit_first_status_must_preserve_a_startup_ui_edit() {
    run_probe(|app, events| {
        let mut findings = Vec::new();
        let native = ImeSettings::default();
        for selected in [
            LlmTemperaturePreset::Focused,
            LlmTemperaturePreset::Expressive,
        ] {
            app.ime_settings_sync = Default::default();
            app.last_native_ime_settings = None;
            app.panel.as_mut().unwrap().chrome.llm_temperature = LlmTemperaturePreset::Balanced;
            app.settings.as_mut().unwrap().chrome.llm_temperature = LlmTemperaturePreset::Balanced;
            click_setting(
                app,
                events,
                InteractionKind::SetLlmTemperature(selected),
                "Tone",
            );
            assert_eq!(app.panel.as_ref().unwrap().chrome.llm_temperature, selected);
            assert_eq!(load_display_settings().unwrap().llm_temperature, selected);
            assert!(
                app.ime_settings_sync
                    .next_patch(&app.panel.as_ref().unwrap().chrome)
                    .is_none()
            );
            app.user_event(
                events,
                PanelUserEvent::InputMethodSettingsChanged(native.clone()),
            );
            let actual = app.panel.as_ref().unwrap().chrome.llm_temperature;
            let pending = app
                .ime_settings_sync
                .next_patch(&app.panel.as_ref().unwrap().chrome);
            if actual != selected
                || pending.and_then(|patch| patch.temperature_tenths) != Some(selected.tenths())
            {
                findings.push(format!("N07 first status after {selected:?} click: panel={actual:?}, settings={:?}, saved={:?}, next_patch={pending:?}",
                    app.settings.as_ref().unwrap().chrome.llm_temperature, load_display_settings().unwrap().llm_temperature));
            }
            // The same click survives once an initial status has been observed.
            app.ime_settings_sync = Default::default();
            app.apply_native_settings(native.clone(), false);
            click_setting(
                app,
                events,
                InteractionKind::SetLlmTemperature(selected),
                "Tone",
            );
            app.apply_native_settings(native.clone(), false);
            let patch = app
                .ime_settings_sync
                .next_patch(&app.panel.as_ref().unwrap().chrome)
                .unwrap();
            assert_eq!(patch.temperature_tenths, Some(selected.tenths()));
            let mut acknowledged = native.clone();
            patch.apply(&mut acknowledged).unwrap();
            app.finish_panel_ime_settings(Some(acknowledged));
            assert_eq!(app.panel.as_ref().unwrap().chrome.llm_temperature, selected);
            assert!(
                app.ime_settings_sync
                    .next_patch(&app.panel.as_ref().unwrap().chrome)
                    .is_none()
            );
            println!(
                "PASS: click after initial status survives refresh and acknowledgement: {selected:?}"
            );
        }
        check_settings_scroll_focus_boundaries(app, events);
        check_settings_model_ownership(app, events);
        findings
    });
}

fn check_settings_scroll_focus_boundaries(app: &mut PanelApp, events: &ActiveEventLoop) {
    use crate::app_state::PersistedDisplaySettings;
    use suzaku_map::ime::gpu::SettingsCategory;
    use winit::{
        dpi::{PhysicalPosition, PhysicalSize},
        event::{DeviceId, ElementState, MouseButton, Touch, TouchPhase},
    };

    let settings = app.settings.as_ref().unwrap();
    let window = settings.window.id();
    let original_size = settings.size;
    let original_focus = settings.is_focused;
    let original_cursor = settings.cursor_position;
    let original_settings = settings.chrome.clone();
    let original_panel = app.panel.as_ref().unwrap().chrome.clone();
    assert!(!settings.runs_without_window_focus);
    assert!(!settings.native.occluded);
    assert!(!settings.interaction.settings_scroll_dragging);
    assert!(!settings.chrome.llm_enabled);
    assert!(!original_panel.llm_enabled);
    let settings_source = (
        settings.chrome.seed_text.clone(),
        settings.chrome.translation.clone(),
        settings.engine.snapshot(),
        PersistedDisplaySettings::from(&settings.chrome),
    );
    let panel = app.panel.as_ref().unwrap();
    let panel_source = (
        panel.window.id(),
        panel.chrome.seed_text.clone(),
        panel.chrome.translation.clone(),
        panel.engine.snapshot(),
        PersistedDisplaySettings::from(&panel.chrome),
    );
    let saved_before = [
        display_settings_path(),
        PathBuf::from(std::env::var_os("SUZAKU_IME_CONFIG").unwrap()),
    ]
    .map(|path| {
        let bytes = fs::read(&path).expect("existing private fixture configuration");
        (path, bytes)
    });
    let check_source = |app: &PanelApp| {
        let settings = app.settings.as_ref().unwrap();
        assert_eq!(settings.window.id(), window);
        assert!(settings.report_view.is_none());
        assert_eq!(settings.chrome.seed_text, settings_source.0);
        assert_eq!(settings.chrome.translation, settings_source.1);
        assert_eq!(settings.engine.snapshot(), settings_source.2);
        assert_eq!(
            PersistedDisplaySettings::from(&settings.chrome),
            settings_source.3
        );
        let panel = app.panel.as_ref().unwrap();
        assert_eq!(panel.window.id(), panel_source.0);
        assert_eq!(panel.chrome.seed_text, panel_source.1);
        assert_eq!(panel.chrome.translation, panel_source.2);
        assert_eq!(panel.engine.snapshot(), panel_source.3);
        assert_eq!(
            PersistedDisplaySettings::from(&panel.chrome),
            panel_source.4
        );
    };
    let pointer = |app: &mut PanelApp, touch: bool, phase, at: (f32, f32)| {
        let position = PhysicalPosition::new(f64::from(at.0), f64::from(at.1));
        if touch {
            app.window_event(
                events,
                window,
                WindowEvent::Touch(Touch {
                    device_id: DeviceId::dummy(),
                    phase,
                    location: position,
                    force: None,
                    id: 472,
                }),
            );
        } else {
            if matches!(phase, TouchPhase::Started | TouchPhase::Moved) {
                app.window_event(
                    events,
                    window,
                    WindowEvent::CursorMoved {
                        device_id: DeviceId::dummy(),
                        position,
                    },
                );
            }
            if matches!(phase, TouchPhase::Started | TouchPhase::Ended) {
                app.window_event(
                    events,
                    window,
                    WindowEvent::MouseInput {
                        device_id: DeviceId::dummy(),
                        state: if phase == TouchPhase::Started {
                            ElementState::Pressed
                        } else {
                            ElementState::Released
                        },
                        button: MouseButton::Left,
                    },
                );
            }
        }
    };

    for touch in [false, true] {
        let settings = app.settings.as_mut().unwrap();
        settings.chrome.settings_category = SettingsCategory::Appearance;
        settings.chrome.settings_search_query.clear();
        settings.chrome.settings_search_focused = false;
        settings.chrome.settings_keyboard_focus = None;
        settings.chrome.settings_collapsed_sections.fill(false);
        let requested = PhysicalSize::new(520, 420);
        let _ = settings.window.request_inner_size(requested);
        let actual = settings.window.inner_size();
        assert_eq!(
            actual, requested,
            "the scrollbar fixture needs a real small viewport"
        );
        settings.resize(actual.width, actual.height);
        // Establish a real transition: an already-false focus flag would skip
        // the production loss handler rather than reproduce this Settings bug.
        app.window_event(events, window, WindowEvent::Focused(true));
        let settings = app.settings.as_mut().unwrap();
        assert!(settings.is_focused);
        assert!(!settings.runs_without_window_focus);
        settings
            .render()
            .expect("present the actual Settings surface");
        assert!(settings.interaction.settings_scroll_max_offset > 0.0);
        assert!(settings.interaction.settings_scroll_drag_range > 0.0);
        settings.set_settings_scroll_offset(settings.interaction.settings_scroll_max_offset * 0.35);
        settings
            .render()
            .expect("present an interior scrollbar position");
        let scene = settings.last_scene.as_ref().unwrap();
        let handle = scene
            .interactive_targets
            .iter()
            .find(|target| target.kind == InteractionKind::SettingsScrollHandle)
            .expect("the presented scrollbar handle must be reachable")
            .rect;
        let at = (handle[0] + handle[2] * 0.5, handle[1] + handle[3] * 0.5);
        assert_eq!(
            scene.hit_interaction(at.0, at.1),
            Some(InteractionKind::SettingsScrollHandle)
        );
        let initial_offset = settings.chrome.settings_scroll_offset;
        let delta = (settings.interaction.settings_scroll_drag_range * 0.1).min(8.0);
        assert!(initial_offset > 0.0);
        assert!(initial_offset < settings.interaction.settings_scroll_max_offset);
        assert!(delta > 0.0);
        pointer(app, touch, TouchPhase::Started, at);
        let settings = app.settings.as_ref().unwrap();
        assert!(settings.interaction.settings_scroll_dragging);
        assert_eq!(
            settings.interaction.settings_scroll_drag_start_y,
            Some(at.1)
        );
        assert_eq!(
            settings.interaction.settings_scroll_drag_start_offset,
            initial_offset
        );
        app.window_event(events, window, WindowEvent::Focused(true));
        assert!(
            app.settings
                .as_ref()
                .unwrap()
                .interaction
                .settings_scroll_dragging,
            "duplicate focused=true must not retire a valid drag"
        );
        pointer(app, touch, TouchPhase::Moved, (at.0, at.1 + delta));
        let retained_offset = app.settings.as_ref().unwrap().chrome.settings_scroll_offset;
        assert!(
            retained_offset > initial_offset,
            "the original drag must actually scroll: touch={touch}"
        );
        check_source(app);

        app.window_event(events, window, WindowEvent::Focused(false));
        assert!(!app.settings.as_ref().unwrap().is_focused);
        // A concrete release may arrive at an unfocused Settings window and be
        // filtered. The reproducer does not assume the backend drops it outright.
        pointer(app, touch, TouchPhase::Ended, (at.0, at.1 + delta));
        assert_eq!(
            app.settings.as_ref().unwrap().chrome.settings_scroll_offset,
            retained_offset
        );
        app.window_event(events, window, WindowEvent::Focused(true));
        assert!(app.settings.as_ref().unwrap().is_focused);
        if touch {
            pointer(app, true, TouchPhase::Moved, (at.0, at.1 + delta * 2.0));
            assert_eq!(
                app.settings.as_ref().unwrap().chrome.settings_scroll_offset,
                retained_offset
            );
        }
        // Blur already clears touch ownership. A later ordinary mouse move is
        // the externally observable ghost scroll for both gesture origins.
        pointer(app, false, TouchPhase::Moved, (at.0, at.1 + delta * 2.0));
        let settings = app.settings.as_ref().unwrap();
        assert_eq!(
            settings.chrome.settings_scroll_offset, retained_offset,
            "focus return without a fresh press must not continue an old scroll drag: touch={touch}"
        );
        assert!(!settings.interaction.settings_scroll_dragging);
        assert_eq!(settings.interaction.settings_scroll_drag_start_y, None);
        assert_eq!(settings.interaction.settings_scroll_drag_start_offset, 0.0);
        assert!(settings.interaction.touch_owner.is_none());
        check_source(app);

        let settings = app.settings.as_mut().unwrap();
        settings
            .render()
            .expect("present a fresh scrollbar gesture");
        let handle = settings
            .last_scene
            .as_ref()
            .unwrap()
            .interactive_targets
            .iter()
            .find(|target| target.kind == InteractionKind::SettingsScrollHandle)
            .unwrap()
            .rect;
        let at = (handle[0] + handle[2] * 0.5, handle[1] + handle[3] * 0.5);
        let before_fresh = settings.chrome.settings_scroll_offset;
        pointer(app, touch, TouchPhase::Started, at);
        assert!(
            app.settings
                .as_ref()
                .unwrap()
                .interaction
                .settings_scroll_dragging
        );
        pointer(app, touch, TouchPhase::Moved, (at.0, at.1 + delta));
        let after_fresh = app.settings.as_ref().unwrap().chrome.settings_scroll_offset;
        assert!(
            after_fresh > before_fresh,
            "a fresh drag must remain usable: touch={touch}"
        );
        pointer(app, touch, TouchPhase::Ended, (at.0, at.1 + delta));
        let settings = app.settings.as_ref().unwrap();
        assert_eq!(settings.chrome.settings_scroll_offset, after_fresh);
        assert!(!settings.interaction.settings_scroll_dragging);
        assert_eq!(settings.interaction.settings_scroll_drag_start_y, None);
        assert_eq!(settings.interaction.settings_scroll_drag_start_offset, 0.0);
        assert!(settings.interaction.touch_owner.is_none());
        check_source(app);
    }

    app.window_event(events, window, WindowEvent::Focused(original_focus));
    let settings = app.settings.as_mut().unwrap();
    settings.chrome = original_settings;
    settings.cursor_position = original_cursor;
    let _ = settings.window.request_inner_size(original_size);
    let actual = settings.window.inner_size();
    assert_eq!(actual, original_size);
    settings.resize(actual.width, actual.height);
    app.panel.as_mut().unwrap().chrome = original_panel;
    check_source(app);
    for (path, bytes) in saved_before {
        assert_eq!(
            fs::read(path).unwrap(),
            bytes,
            "focus/scroll gestures must not persist settings"
        );
    }
    println!(
        "PASS: controlled real Settings mouse/touch drags stop on focus loss despite filtered releases, stay inert on return without a press, and permit fresh drags without changing source or preferences"
    );
}

fn check_settings_model_ownership(app: &mut PanelApp, events: &ActiveEventLoop) {
    use suzaku_map::ime::PredictionStatus;

    // A valid, disabled local configuration is acknowledged by the main window.
    // Empty drafts cannot schedule predictions: no model or credential is used.
    let confirmed = ImeSettings::default();
    confirmed.save().unwrap();
    app.ime_settings_sync = Default::default();
    app.apply_native_settings(confirmed.clone(), false);
    assert!(app.panel.as_ref().unwrap().chrome.seed_text.is_empty());
    assert!(
        app.panel
            .as_ref()
            .unwrap()
            .engine
            .snapshot()
            .seed_text
            .is_empty()
    );
    assert!(
        app.settings
            .as_ref()
            .unwrap()
            .confirmed_model_config
            .is_none()
    );
    for with_error in [true, false] {
        let settings = app.settings.as_mut().unwrap();
        settings.is_focused = true;
        settings.last_commit_feedback = None;
        settings.commit_feedback_ticks = 0;
        if with_error {
            app.show_input_method_error("synthetic host failure; retry explicitly".into());
        }
        let settings = app.settings.as_mut().unwrap();
        crate::native_sync::present_test_frame(settings);
        assert_eq!(
            settings
                .build_commit_feedback(settings.last_scene.as_ref().unwrap())
                .is_some(),
            with_error
        );
        let feedback = settings.last_commit_feedback.clone();
        let ticks = settings.commit_feedback_ticks;
        let engine = settings.engine.snapshot();
        let candidates = settings.engine.candidates().to_vec();
        let seed = settings.chrome.seed_text.clone();
        for (target, search, enabled) in [
            (InteractionKind::SetLlmEnabled(true), "LLM", true),
            (
                InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Focused),
                "Tone",
                true,
            ),
            (
                InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Expressive),
                "Tone",
                true,
            ),
            (InteractionKind::SetLlmEnabled(false), "LLM", false),
        ] {
            let before = app
                .settings
                .as_ref()
                .unwrap()
                .chrome
                .prediction_edit_generation;
            click_setting(app, events, target, search);
            let settings = app.settings.as_ref().unwrap();
            assert_eq!(
                settings.last_commit_feedback, feedback,
                "model controls must preserve settings-owned feedback: {target:?}, with_error={with_error}"
            );
            assert_eq!(settings.commit_feedback_ticks, ticks);
            assert_eq!(settings.engine.snapshot(), engine, "{target:?}");
            assert_eq!(settings.engine.candidates(), candidates, "{target:?}");
            assert_eq!(settings.chrome.seed_text, seed, "{target:?}");
            assert_eq!(
                settings.engine.prediction_status(),
                PredictionStatus::Disabled
            );
            let scene = settings.last_scene.as_ref().unwrap();
            assert_eq!(settings.build_commit_feedback(scene).is_some(), with_error);
            let panel = app.panel.as_ref().unwrap();
            assert_eq!(panel.chrome.llm_enabled, enabled);
            assert_eq!(settings.chrome.llm_enabled, enabled);
            assert_eq!(
                panel.engine.prediction_status(),
                if enabled {
                    PredictionStatus::Idle
                } else {
                    PredictionStatus::Disabled
                }
            );
            assert!(
                panel.last_commit_feedback.is_none(),
                "valid main configuration: {target:?}"
            );
            assert_eq!(
                panel.confirmed_model_config.as_ref(),
                Some(&confirmed.provider)
            );
            let mut expected_generation = before;
            let channel = match target {
                InteractionKind::SetLlmEnabled(_) => 0,
                InteractionKind::SetLlmTemperature(temperature) => {
                    assert_eq!(panel.chrome.llm_temperature, temperature);
                    assert_eq!(settings.chrome.llm_temperature, temperature);
                    1
                }
                _ => unreachable!(),
            };
            expected_generation[channel] = expected_generation[channel].wrapping_add(1);
            assert_eq!(
                settings.chrome.prediction_edit_generation,
                expected_generation
            );
            assert_eq!(panel.chrome.prediction_edit_generation, expected_generation);
        }
    }
    // The auxiliary guard must not hide a genuine missing configuration in Main.
    let panel = app.panel.as_mut().unwrap();
    panel.confirmed_model_config = None;
    panel.chrome.llm_enabled = true;
    panel.reconfigure_model_provider();
    assert_eq!(
        panel.last_commit_feedback.as_deref(),
        Some("Check the configured model and endpoint.")
    );
    assert_eq!(panel.engine.prediction_status(), PredictionStatus::Disabled);
    panel.confirmed_model_config = Some(confirmed.provider);
    panel.reconfigure_model_provider();
    assert!(panel.last_commit_feedback.is_none());
    assert_eq!(panel.engine.prediction_status(), PredictionStatus::Idle);
    println!(
        "PASS: settings model controls retain service errors and leave prediction ownership in Main; no model request"
    );
}

struct RestorePermissions {
    path: PathBuf,
    permissions: fs::Permissions,
}

impl Drop for RestorePermissions {
    fn drop(&mut self) {
        fs::set_permissions(&self.path, self.permissions.clone()).unwrap();
    }
}

#[test]
#[ignore = "requires private Xvfb and non-root file permissions; run by test-settings-chain-audit.sh in Linux CI"]
fn audit_display_save_failure_must_be_visible_or_rolled_back() {
    run_probe(|app, events| {
        use suzaku_map::ime::gpu::ThemePreset;
        app.apply_native_settings(ImeSettings::default(), false);
        let panel = app.panel.as_mut().unwrap();
        panel.chrome.theme_preset = ThemePreset::Suzaku;
        panel.persist_display_settings();
        panel.last_commit_feedback = None;
        let path = display_settings_path();
        let before = fs::read(&path).unwrap();
        let parent = path.parent().unwrap().to_path_buf();
        let restore = RestorePermissions {
            permissions: fs::metadata(&parent).unwrap().permissions(),
            path: parent.clone(),
        };
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
        assert!(
            app_state::save_display_settings(&app_state::PersistedDisplaySettings::from(
                &panel.chrome
            ))
            .is_err(),
            "fixture must reject writes (do not run as root)"
        );
        click_setting(
            app,
            events,
            InteractionKind::SetThemePreset(ThemePreset::Baihu),
            ThemePreset::Baihu.label(),
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "failed save must preserve old bytes"
        );
        let panel = app.panel.as_ref().unwrap();
        let saved = load_display_settings().unwrap().theme_preset;
        let reopened_window = Arc::new(events.create_window(panel_window_attributes()).unwrap());
        let reopened = pollster::block_on(PanelState::new(reopened_window)).unwrap();
        assert_eq!(
            reopened.chrome.theme_preset, saved,
            "a newly created panel loads the old saved theme"
        );
        let mut findings = Vec::new();
        if panel.chrome.theme_preset != saved
            && panel.last_commit_feedback.is_none()
            && app
                .settings
                .as_ref()
                .unwrap()
                .last_commit_feedback
                .is_none()
        {
            findings.push(format!("N09 failed appearance save: panel={:?}, settings={:?}, reloaded={saved:?}, both error messages absent",
                panel.chrome.theme_preset, app.settings.as_ref().unwrap().chrome.theme_preset));
        }
        assert!(app.panel.as_ref().unwrap().chrome.settings_save_failed);
        assert!(app.settings.as_ref().unwrap().chrome.settings_save_failed);
        let scene = app.settings.as_mut().unwrap().current_scene();
        assert!(
            scene
                .interactive_targets
                .iter()
                .any(|item| item.kind == InteractionKind::RetrySaveSettings)
        );
        drop(restore);
        // Redrawing after the filesystem recovers is not a request to keep writing.
        for _ in 0..3 {
            let id = app.settings.as_ref().unwrap().window.id();
            app.window_event(events, id, WindowEvent::RedrawRequested);
            assert_eq!(fs::read(&path).unwrap(), before);
        }
        click_setting(app, events, InteractionKind::RetrySaveSettings, "");
        assert_eq!(
            load_display_settings().unwrap().theme_preset,
            ThemePreset::Baihu
        );
        assert!(!app.panel.as_ref().unwrap().chrome.settings_save_failed);
        assert!(!app.settings.as_ref().unwrap().chrome.settings_save_failed);
        assert!(app.panel.as_ref().unwrap().last_commit_feedback.is_none());
        let scene = app.settings.as_mut().unwrap().current_scene();
        assert!(
            !scene
                .interactive_targets
                .iter()
                .any(|item| item.kind == InteractionKind::RetrySaveSettings)
        );
        click_setting(
            app,
            events,
            InteractionKind::SetThemePreset(ThemePreset::Qinglong),
            ThemePreset::Qinglong.label(),
        );
        assert_eq!(
            load_display_settings().unwrap().theme_preset,
            ThemePreset::Qinglong
        );
        println!(
            "PASS: failed save is visible in both windows, old bytes survive, no redraw retry loop; explicit Retry saves the preview and clears the warning"
        );
        findings
    });
}

#[test]
#[ignore = "requires private Xvfb and non-root file permissions; run by test-settings-chain-audit.sh in Linux CI"]
fn audit_reopened_settings_must_preserve_the_unsaved_preview() {
    run_probe(|app, events| {
        use crate::app_state::PersistedDisplaySettings;
        use suzaku_map::ime::gpu::ThemePreset;

        app.apply_native_settings(ImeSettings::default(), false);
        let panel = app.panel.as_mut().unwrap();
        panel.chrome.theme_preset = ThemePreset::Suzaku;
        panel.persist_display_settings();
        let path = display_settings_path();
        let before = fs::read(&path).unwrap();
        let parent = path.parent().unwrap().to_path_buf();
        let restore = RestorePermissions {
            permissions: fs::metadata(&parent).unwrap().permissions(),
            path: parent.clone(),
        };
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
        assert!(
            app_state::save_display_settings(&PersistedDisplaySettings::from(&panel.chrome))
                .is_err(),
            "fixture must reject writes (do not run as root)"
        );
        click_setting(
            app,
            events,
            InteractionKind::SetThemePreset(ThemePreset::Baihu),
            ThemePreset::Baihu.label(),
        );
        let preview = PersistedDisplaySettings::from(&app.panel.as_ref().unwrap().chrome);
        assert_eq!(preview.theme_preset, ThemePreset::Baihu);
        assert!(!preview.llm_enabled);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(app.panel.as_ref().unwrap().chrome.settings_save_failed);
        assert!(app.settings.as_ref().unwrap().chrome.settings_save_failed);

        let closed_id = app.settings.as_ref().unwrap().window.id();
        app.window_event(events, closed_id, WindowEvent::CloseRequested);
        assert!(app.settings.is_none());
        app.open_settings(events);
        let settings = app.settings.as_ref().unwrap();
        let reopened = PersistedDisplaySettings::from(&settings.chrome);
        let mut findings = Vec::new();
        if reopened != preview {
            findings.push(format!(
                "reopened Settings replaced the unsaved preview: expected={preview:?}, actual={reopened:?}"
            ));
        }
        assert!(settings.chrome.settings_save_failed);
        let reopened_id = settings.window.id();
        app.window_event(events, reopened_id, WindowEvent::RedrawRequested);
        for (name, state) in [
            ("panel", app.panel.as_ref().unwrap()),
            ("settings", app.settings.as_ref().unwrap()),
        ] {
            let actual = PersistedDisplaySettings::from(&state.chrome);
            if actual != preview {
                findings.push(format!(
                    "first Settings event changed {name}: expected={preview:?}, actual={actual:?}"
                ));
            }
            assert!(state.chrome.settings_save_failed);
            assert!(!state.chrome.llm_enabled);
        }
        assert_eq!(fs::read(&path).unwrap(), before);
        let after_first_event = app.panel.as_ref().unwrap().chrome.theme_preset;
        drop(restore);

        // Only the explicit Retry may persist the retained preview after recovery.
        click_setting(app, events, InteractionKind::RetrySaveSettings, "");
        let saved = load_display_settings().unwrap();
        if saved != preview {
            findings.push(format!(
                "Retry saved a stale configuration: expected={preview:?}, actual={saved:?}"
            ));
        }
        assert!(!app.panel.as_ref().unwrap().chrome.settings_save_failed);
        assert!(!app.settings.as_ref().unwrap().chrome.settings_save_failed);
        assert!(app.panel.as_ref().unwrap().last_commit_feedback.is_none());
        assert!(app.tray.is_none());
        assert!(app.native_sync.is_none());
        assert!(
            app.ime_settings_sync
                .next_patch(&app.panel.as_ref().unwrap().chrome)
                .is_none(),
            "an appearance-only edit must not enqueue a native settings write"
        );
        println!(
            "reopened Settings evidence: disk_before=Suzaku, preview={:?}, immediately_reopened={:?}, after_first_event={:?}, retried_disk={:?}",
            preview.theme_preset, reopened.theme_preset, after_first_event, saved.theme_preset
        );
        for reuse_window in [true, false] {
            if !reuse_window {
                let id = app.settings.as_ref().unwrap().window.id();
                app.window_event(events, id, WindowEvent::CloseRequested);
                assert!(app.settings.is_none());
            }
            let panel = app.panel.as_mut().unwrap();
            panel.last_compact_toggle = None;
            panel.apply_compact_mode(true);
            assert!(panel.chrome.compact_mode);
            // A deliberate OpenSettings request is not another bubble tap.
            panel.last_compact_toggle = Some(Instant::now());
            app.open_settings(events);
            assert!(
                !app.panel.as_ref().unwrap().chrome.compact_mode,
                "explicit OpenSettings must bypass fold debounce (reuse={reuse_window})"
            );
            assert!(app.panel.as_ref().unwrap().chrome.settings_open);
            let settings = app.settings.as_ref().unwrap();
            assert!(settings.chrome.settings_open);
            let id = settings.window.id();
            app.window_event(events, id, WindowEvent::RedrawRequested);
            assert!(app.settings.is_some());
            assert_eq!(
                app.panel.as_ref().unwrap().chrome.theme_preset,
                ThemePreset::Baihu
            );
        }
        findings
    });
}

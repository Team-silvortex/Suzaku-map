//! Apply shared panel settings and persist their display preferences.
use super::{PanelState, PanelWindowKind};
use crate::app_state::{PersistedDisplaySettings, apply_display_settings, save_display_settings};
use suzaku_map::ime::gpu::PanelChromeState;

struct SettingsUpdatePlan {
    incoming_settings: PersistedDisplaySettings,
    settings_changed: bool,
    needs_font_rebuild: bool,
    needs_llm_reconfigure: bool,
    shortcuts_changed: bool,
}

impl SettingsUpdatePlan {
    fn between(current: &PanelChromeState, other: &PanelChromeState) -> Self {
        let current_settings = PersistedDisplaySettings::from(current);
        let incoming_settings = PersistedDisplaySettings::from(other);
        let settings_changed = current_settings != incoming_settings;
        let needs_font_rebuild =
            current.font_face != other.font_face || current.text_smoothing != other.text_smoothing;
        let needs_llm_reconfigure = current.llm_enabled != other.llm_enabled
            || current.llm_model != other.llm_model
            || current.llm_temperature != other.llm_temperature;
        let shortcuts_changed = current.shortcut_profile != other.shortcut_profile
            || current.shortcut_bindings != other.shortcut_bindings;
        Self {
            incoming_settings,
            settings_changed,
            needs_font_rebuild,
            needs_llm_reconfigure,
            shortcuts_changed,
        }
    }
}

impl PanelState {
    pub(super) fn persist_display_settings(&mut self) {
        if self.kind == PanelWindowKind::Settings {
            return;
        }
        let mut settings = PersistedDisplaySettings::from(&self.chrome);
        settings.window_scale = self.window_scale;
        const MESSAGE: &str = "Settings not saved. Check folder permissions and retry in Settings.";
        match save_display_settings(&settings) {
            Ok(()) => {
                self.chrome.settings_save_failed = false;
                if self.last_commit_feedback.as_deref() == Some(MESSAGE) {
                    self.last_commit_feedback = None;
                    self.commit_feedback_ticks = 0;
                }
            }
            Err(error) => {
                eprintln!("Suzaku panel settings were not saved: {error}");
                self.chrome.settings_save_failed = true;
                self.last_commit_feedback = Some(MESSAGE.into());
                self.commit_feedback_ticks = 180;
            }
        }
        self.last_scene = None;
    }

    pub(super) fn adopt_settings_from(
        &mut self,
        other: &suzaku_map::ime::gpu::PanelChromeState,
    ) -> bool {
        let SettingsUpdatePlan {
            incoming_settings,
            settings_changed,
            needs_font_rebuild,
            needs_llm_reconfigure,
            shortcuts_changed,
        } = SettingsUpdatePlan::between(&self.chrome, other);
        self.chrome.prediction_edit_generation = other.prediction_edit_generation;
        self.chrome.shortcut_profile = other.shortcut_profile;
        self.chrome.shortcut_bindings = other.shortcut_bindings;
        self.chrome.shortcut_edit_generation = other.shortcut_edit_generation;

        if settings_changed {
            let previous_layout = self.chrome.effective_panel_layout();
            apply_display_settings(&mut self.chrome, &incoming_settings);
            self.apply_panel_layout_change(previous_layout);
            self.apply_window_decorations();
            self.persist_display_settings();
        }
        self.chrome.settings_search_query = other.settings_search_query.clone();
        self.chrome.settings_category = other.settings_category;
        self.chrome.settings_search_focused = other.settings_search_focused;
        self.chrome.settings_keyboard_focus = other.settings_keyboard_focus;
        self.chrome.settings_collapsed_sections = other.settings_collapsed_sections.clone();
        self.chrome.settings_scroll_offset = other.settings_scroll_offset;

        if settings_changed && needs_font_rebuild {
            self.rebuild_font_atlas();
        }
        if settings_changed && self.kind == PanelWindowKind::Main && needs_llm_reconfigure {
            self.reconfigure_model_provider();
        }
        settings_changed || shortcuts_changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use suzaku_map::ime::gpu::{
        CandidateDensity, DisplayTextScale, FontFaceChoice, InteractionKind, LlmModelPreset,
        LlmTemperaturePreset, PanelLayoutMode, PreviewStyle, SettingsCategory, TextSmoothing,
        TextSpacing, ThemePreset,
    };
    use suzaku_map::ime::shortcuts::{
        ShortcutAction, ShortcutBindings, ShortcutChord, ShortcutProfile,
    };
    use suzaku_map::ui::UiLanguage;

    type ChromeEdit = (&'static str, fn(&mut PanelChromeState));

    fn update_flags(plan: &SettingsUpdatePlan) -> [bool; 4] {
        [
            plan.settings_changed,
            plan.needs_font_rebuild,
            plan.needs_llm_reconfigure,
            plan.shortcuts_changed,
        ]
    }

    #[test]
    fn unchanged_settings_do_not_schedule_updates() {
        let chrome = PanelChromeState::default();
        let plan = SettingsUpdatePlan::between(&chrome, &chrome);
        assert_eq!(update_flags(&plan), [false; 4]);
        assert!(!(plan.settings_changed || plan.shortcuts_changed));
        assert_eq!(
            plan.incoming_settings,
            PersistedDisplaySettings::from(&chrome)
        );
    }

    #[test]
    fn all_eighteen_persisted_fields_keep_their_update_scope() {
        let current = PanelChromeState::default();
        let LlmModelPreset::Configured = current.llm_model;
        let PersistedDisplaySettings {
            text_scale: _,
            candidate_density: _,
            preview_style: _,
            font_face: _,
            text_spacing: _,
            text_smoothing: _,
            theme_preset: _,
            hide_system_titlebar: _,
            panel_layout_mode: _,
            ui_language: _,
            voice_auto_insert: _,
            llm_enabled: _,
            llm_model: _,
            llm_temperature: _,
            pointer_tap_slop_tenths: _,
            pointer_tap_max_ms: _,
            pointer_target_slop_tenths: _,
            window_scale: _,
        } = PersistedDisplaySettings::from(&current);
        let edits: [ChromeEdit; 18] = [
            ("text_scale", |chrome| {
                chrome.text_scale = DisplayTextScale::Large
            }),
            ("candidate_density", |chrome| {
                chrome.candidate_density = CandidateDensity::Compact
            }),
            ("preview_style", |chrome| {
                chrome.preview_style = PreviewStyle::Full
            }),
            ("font_face", |chrome| {
                chrome.font_face = FontFaceChoice::Monaco
            }),
            ("text_spacing", |chrome| {
                chrome.text_spacing = TextSpacing::Relaxed
            }),
            ("text_smoothing", |chrome| {
                chrome.text_smoothing = TextSmoothing::Sharp
            }),
            ("theme_preset", |chrome| {
                chrome.theme_preset = ThemePreset::Baihu
            }),
            ("hide_system_titlebar", |chrome| {
                chrome.hide_system_titlebar = true
            }),
            ("panel_layout_mode", |chrome| {
                chrome.panel_layout_mode = PanelLayoutMode::BottomDock
            }),
            ("ui_language", |chrome| {
                chrome.ui_language = UiLanguage::Japanese
            }),
            ("voice_auto_insert", |chrome| {
                chrome.voice_auto_insert = false
            }),
            ("llm_enabled", |chrome| chrome.llm_enabled = true),
            // Configured is currently the only model value; its legacy name is an alias.
            ("llm_model", |chrome| {
                chrome.llm_model = LlmModelPreset::Llama32_3b
            }),
            ("llm_temperature", |chrome| {
                chrome.llm_temperature = LlmTemperaturePreset::Expressive
            }),
            ("pointer_tap_slop_tenths", |chrome| {
                chrome.pointer_tap_slop_tenths = 80
            }),
            ("pointer_tap_max_ms", |chrome| {
                chrome.pointer_tap_max_ms = 600
            }),
            ("pointer_target_slop_tenths", |chrome| {
                chrome.pointer_target_slop_tenths = 40
            }),
            ("window_scale", |chrome| chrome.window_scale = 1.25),
        ];
        for (field, edit) in edits {
            let mut incoming = current.clone();
            edit(&mut incoming);
            let plan = SettingsUpdatePlan::between(&current, &incoming);
            let expected = match field {
                "font_face" | "text_smoothing" => [true, true, false, false],
                "llm_enabled" | "llm_temperature" => [true, false, true, false],
                "llm_model" => [false; 4],
                _ => [true, false, false, false],
            };
            assert_eq!(update_flags(&plan), expected, "{field}");
            assert_eq!(
                plan.incoming_settings,
                PersistedDisplaySettings::from(&incoming),
                "{field}"
            );
            assert_eq!(
                plan.settings_changed || plan.shortcuts_changed,
                field != "llm_model",
                "{field}"
            );
        }
    }

    #[test]
    fn shortcuts_request_propagation_without_display_or_provider_updates() {
        let edits: [ChromeEdit; 2] = [
            ("shortcut_profile", |chrome| {
                chrome.shortcut_profile = ShortcutProfile::HomeRow
            }),
            ("shortcut_bindings", |chrome| {
                chrome.shortcut_bindings = ShortcutBindings::default().with_binding(
                    ShortcutAction::Next,
                    Some(ShortcutChord::parse("Ctrl+K").unwrap()),
                )
            }),
        ];
        let current = PanelChromeState::default();
        for (field, edit) in edits {
            let mut incoming = current.clone();
            edit(&mut incoming);
            let plan = SettingsUpdatePlan::between(&current, &incoming);
            assert_eq!(update_flags(&plan), [false, false, false, true], "{field}");
            assert!(plan.settings_changed || plan.shortcuts_changed, "{field}");
            assert_eq!(
                plan.incoming_settings,
                PersistedDisplaySettings::from(&current)
            );
        }
    }

    #[test]
    fn transient_navigation_and_edit_state_do_not_schedule_settings_updates() {
        let edits: [ChromeEdit; 15] = [
            ("prediction_edit_generation", |chrome| {
                chrome.prediction_edit_generation = [7, 9]
            }),
            ("shortcut_edit_generation", |chrome| {
                chrome.shortcut_edit_generation = 11
            }),
            ("settings_search_query", |chrome| {
                chrome.settings_search_query = "Tone".into()
            }),
            ("settings_category", |chrome| {
                chrome.settings_category = SettingsCategory::Model
            }),
            ("settings_search_focused", |chrome| {
                chrome.settings_search_focused = true
            }),
            ("settings_keyboard_focus", |chrome| {
                chrome.settings_keyboard_focus = Some(InteractionKind::SettingsSearchInput)
            }),
            ("settings_collapsed_sections", |chrome| {
                chrome.settings_collapsed_sections = vec![true]
            }),
            ("settings_scroll_offset", |chrome| {
                chrome.settings_scroll_offset = 24.0
            }),
            ("settings_open", |chrome| chrome.settings_open = true),
            ("settings_save_failed", |chrome| {
                chrome.settings_save_failed = true
            }),
            ("shortcut_recording", |chrome| {
                chrome.shortcut_recording = Some(ShortcutAction::Next)
            }),
            ("shortcut_recording_error", |chrome| {
                chrome.shortcut_recording_error = "conflict".into()
            }),
            ("detected_panel_layout", |chrome| {
                chrome.detected_panel_layout = PanelLayoutMode::BottomDock
            }),
            ("seed_text", |chrome| chrome.seed_text = "draft".into()),
            ("caret_index", |chrome| chrome.caret_index = 1),
        ];
        let current = PanelChromeState::default();
        for (field, edit) in edits {
            let mut incoming = current.clone();
            edit(&mut incoming);
            let plan = SettingsUpdatePlan::between(&current, &incoming);
            assert_eq!(update_flags(&plan), [false; 4], "{field}");
            assert!(
                !(plan.settings_changed || plan.shortcuts_changed),
                "{field}"
            );
            assert_eq!(
                plan.incoming_settings,
                PersistedDisplaySettings::from(&current)
            );
        }
    }
}

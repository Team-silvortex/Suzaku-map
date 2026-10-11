#![cfg(feature = "gpu")]

use suzaku_map::{
    ime::{
        gpu::{
            DisplayTextScale, InteractionKind, PanelChromeState, SettingsCategory,
            WgpuCandidateRenderer,
        },
        shortcuts::{ShortcutAction, ShortcutBindings, ShortcutChord, ShortcutProfile},
    },
    ui::UiLanguage,
};

#[test]
fn explicit_default_choices_and_same_value_recordings_each_advance_once() {
    let mut chrome = PanelChromeState::default();
    chrome.change_shortcut_profile(ShortcutProfile::Standard);
    assert_eq!(chrome.shortcut_edit_generation, 1);
    chrome.clear_shortcut_binding(ShortcutAction::Previous);
    assert_eq!(chrome.shortcut_edit_generation, 2);
    chrome.reset_custom_shortcuts();
    assert_eq!(chrome.shortcut_edit_generation, 3);
    for expected in [4, 5] {
        chrome.toggle_shortcut_recording(ShortcutAction::Previous);
        assert!(chrome.record_shortcut_key(u32::from('K'), ShortcutChord::CTRL));
        assert_eq!(chrome.shortcut_edit_generation, expected);
        assert_eq!(
            chrome.shortcut_binding_label(ShortcutAction::Previous),
            "Ctrl+K"
        );
        assert_eq!(chrome.shortcut_recording, None);
    }
}

#[test]
fn recording_toggle_cancel_and_switch_are_ephemeral_and_do_not_edit_drafts() {
    let mut chrome = PanelChromeState {
        seed_text: "草稿 café 😀".into(),
        settings_search_query: "Previous".into(),
        settings_search_focused: true,
        ..Default::default()
    };
    chrome.toggle_shortcut_recording(ShortcutAction::Previous);
    assert_eq!(chrome.shortcut_recording, Some(ShortcutAction::Previous));
    assert!(!chrome.settings_search_focused);
    chrome.toggle_shortcut_recording(ShortcutAction::Next);
    assert_eq!(chrome.shortcut_recording, Some(ShortcutAction::Next));
    chrome.toggle_shortcut_recording(ShortcutAction::Next);
    assert_eq!(chrome.shortcut_recording, None);
    assert!(!chrome.record_shortcut_key(u32::from('x'), ShortcutChord::CTRL));
    chrome.toggle_shortcut_recording(ShortcutAction::Adopt);
    chrome.cancel_shortcut_recording();
    assert_eq!(chrome.shortcut_edit_generation, 0);
    assert_eq!(chrome.shortcut_bindings, ShortcutBindings::default());
    assert_eq!(chrome.seed_text, "草稿 café 😀");
    assert_eq!(chrome.settings_search_query, "Previous");
}

#[test]
fn invalid_and_duplicate_keys_never_save_and_allow_a_corrected_attempt() {
    let mut chrome = PanelChromeState::default();
    for (key, modifiers) in [
        (u32::from('x'), 0),
        (u32::from('x'), ShortcutChord::SHIFT),
        (u32::from('x'), ShortcutChord::CTRL | ShortcutChord::ALT),
        (u32::from('x'), ShortcutChord::CTRL | 8),
        (u32::from(' '), ShortcutChord::ALT),
        (u32::from('é'), ShortcutChord::CTRL),
        (u32::from(','), ShortcutChord::CTRL),
    ] {
        chrome.toggle_shortcut_recording(ShortcutAction::Previous);
        assert!(chrome.record_shortcut_key(key, modifiers));
        assert_eq!(chrome.shortcut_recording, Some(ShortcutAction::Previous));
        assert!(!chrome.shortcut_recording_error.is_empty());
        assert_eq!(chrome.shortcut_edit_generation, 0);
        assert_eq!(chrome.shortcut_bindings, ShortcutBindings::default());
        chrome.cancel_shortcut_recording();
    }
    chrome.toggle_shortcut_recording(ShortcutAction::Previous);
    chrome.record_shortcut_key(u32::from('k'), ShortcutChord::CTRL);
    let saved = chrome.shortcut_bindings;
    chrome.toggle_shortcut_recording(ShortcutAction::Next);
    chrome.record_shortcut_key(u32::from('K'), ShortcutChord::CTRL);
    assert_eq!(chrome.shortcut_bindings, saved);
    assert_eq!(chrome.shortcut_edit_generation, 1);
    assert_eq!(chrome.shortcut_recording, Some(ShortcutAction::Next));
    chrome.record_shortcut_key(u32::from('j'), ShortcutChord::CTRL);
    assert_eq!(chrome.shortcut_edit_generation, 2);
    assert!(chrome.shortcut_recording_error.is_empty());
}

#[test]
fn profile_and_language_reset_conflicts_preserve_the_last_valid_preferences() {
    let mut chrome = PanelChromeState::default();
    chrome.toggle_shortcut_recording(ShortcutAction::Previous);
    chrome.record_shortcut_key(u32::from('j'), ShortcutChord::ALT);
    chrome.change_shortcut_profile(ShortcutProfile::HomeRow);
    assert_eq!(chrome.shortcut_profile, ShortcutProfile::Standard);
    assert_eq!(chrome.shortcut_edit_generation, 1);
    assert!(chrome.shortcut_recording_error.contains("Home row"));
    chrome.clear_shortcut_binding(ShortcutAction::Previous);
    chrome.change_shortcut_profile(ShortcutProfile::HomeRow);
    assert_eq!(chrome.shortcut_profile, ShortcutProfile::HomeRow);
    assert_eq!(chrome.shortcut_edit_generation, 3);
    chrome.toggle_shortcut_recording(ShortcutAction::SwitchLanguage);
    chrome.record_shortcut_key(u32::from('g'), ShortcutChord::CTRL);
    chrome.toggle_shortcut_recording(ShortcutAction::Adopt);
    chrome.record_shortcut_key(u32::from(' '), ShortcutChord::CTRL | ShortcutChord::SHIFT);
    let saved = chrome.shortcut_bindings;
    let generation = chrome.shortcut_edit_generation;
    chrome.clear_shortcut_binding(ShortcutAction::SwitchLanguage);
    assert_eq!(chrome.shortcut_bindings, saved);
    assert_eq!(chrome.shortcut_edit_generation, generation);
    assert!(!chrome.shortcut_recording_error.is_empty());
    chrome.reset_custom_shortcuts();
    assert_eq!(chrome.shortcut_bindings, ShortcutBindings::default());
    assert_eq!(chrome.shortcut_profile, ShortcutProfile::HomeRow);
    assert_eq!(chrome.shortcut_edit_generation, generation + 1);
}

#[test]
fn all_six_controls_are_keyboard_accessible_and_references_stay_read_only() {
    for language in UiLanguage::ALL {
        for profile in ShortcutProfile::ALL {
            let chrome = PanelChromeState {
                ui_language: language,
                shortcut_profile: profile,
                settings_category: SettingsCategory::Shortcuts,
                ..Default::default()
            };
            let scene =
                WgpuCandidateRenderer::new(620.0, 2400.0).build_settings_scene(&chrome, None);
            for action in ShortcutAction::ALL {
                for kind in [
                    InteractionKind::RecordShortcut(action),
                    InteractionKind::ClearShortcutBinding(action),
                ] {
                    assert!(
                        scene
                            .settings_focus_targets
                            .iter()
                            .any(|target| target.kind == kind)
                    );
                }
                assert!(!chrome.shortcut_binding_label(action).is_empty());
            }
            assert!(
                scene
                    .settings_focus_targets
                    .iter()
                    .any(|target| target.kind == InteractionKind::ResetCustomShortcuts)
            );
            assert!(
                scene
                    .interactive_targets
                    .iter()
                    .chain(&scene.settings_focus_targets)
                    .all(|target| !matches!(target.kind, InteractionKind::ShortcutReference(_)))
            );
        }
    }
}

#[test]
fn language_reference_tracks_the_effective_custom_key_in_every_ui_language() {
    for language in UiLanguage::ALL {
        let mut chrome = PanelChromeState {
            ui_language: language,
            settings_search_query: "Switch EN/ZH".into(),
            ..Default::default()
        };
        chrome.toggle_shortcut_recording(ShortcutAction::SwitchLanguage);
        chrome.record_shortcut_key(u32::from('g'), ShortcutChord::CTRL | ShortcutChord::SHIFT);
        let scene = WgpuCandidateRenderer::new(620.0, 600.0).build_settings_scene(&chrome, None);
        let lines: Vec<_> = scene
            .text_sections
            .iter()
            .flat_map(|section| &section.layouts)
            .flat_map(|layout| &layout.lines)
            .map(String::as_str)
            .collect();
        assert_eq!(
            lines.iter().filter(|line| **line == "Ctrl+Shift+G").count(),
            2
        );
        assert!(!lines.contains(&"Ctrl+Shift+Space"));
        chrome.clear_shortcut_binding(ShortcutAction::SwitchLanguage);
        assert_eq!(
            chrome.shortcut_binding_label(ShortcutAction::SwitchLanguage),
            "Ctrl+Shift+Space"
        );
    }
}

#[test]
fn active_recording_and_errors_use_localized_labels_without_mutating_bindings() {
    for language in UiLanguage::ALL {
        let mut chrome = PanelChromeState {
            ui_language: language,
            settings_search_query: "Previous".into(),
            ..Default::default()
        };
        chrome.toggle_shortcut_recording(ShortcutAction::Previous);
        chrome.record_shortcut_key(u32::from('x'), 0);
        let scene = WgpuCandidateRenderer::new(620.0, 600.0).build_settings_scene(&chrome, None);
        assert!(
            scene
                .text_sections
                .iter()
                .flat_map(|section| &section.layouts)
                .flat_map(|layout| &layout.lines)
                .any(|line| line == language.tr("Press keys · Esc cancels"))
        );
        assert_eq!(chrome.shortcut_bindings, ShortcutBindings::default());
        assert_eq!(chrome.shortcut_edit_generation, 0);
        assert_ne!(language.tr("Custom keys"), "");
        assert!(
            !language
                .message(&chrome.shortcut_recording_error)
                .is_empty()
        );
    }
}

#[test]
fn narrow_large_settings_show_editable_entry_first_and_wrap_notice_text_completely() {
    fn plain(text: &str) -> String {
        text.chars().filter(|ch| !ch.is_whitespace()).collect()
    }
    for language in UiLanguage::ALL {
        let mut chrome = PanelChromeState {
            ui_language: language,
            text_scale: DisplayTextScale::Large,
            settings_category: SettingsCategory::Shortcuts,
            ..Default::default()
        };
        let initial = WgpuCandidateRenderer::new(400.0, 540.0).build_settings_scene(&chrome, None);
        assert!(
            initial
                .interactive_targets
                .iter()
                .any(|target| target.kind
                    == InteractionKind::RecordShortcut(ShortcutAction::Previous)),
            "editable entry must not be buried behind references: {language:?}"
        );
        chrome.settings_search_query = "Previous".into();
        chrome.toggle_shortcut_recording(ShortcutAction::Previous);
        chrome.record_shortcut_key(u32::from('x'), ShortcutChord::CTRL | ShortcutChord::ALT);
        let error = language.message(&chrome.shortcut_recording_error);
        let scene = WgpuCandidateRenderer::new(400.0, 1200.0).build_settings_scene(&chrome, None);
        assert!(
            !scene
                .settings_option_truncated
                .contains(&InteractionKind::RecordShortcut(ShortcutAction::Previous)),
            "recording label must remain readable at 400px/Large: {language:?}"
        );
        let layout = scene
            .text_sections
            .iter()
            .flat_map(|section| &section.layouts)
            .find(|layout| plain(&layout.lines.join("")) == plain(&error))
            .expect("the entire localized error wraps even while filtering the recording row");
        assert!(!layout.truncated);
        assert!(layout.lines.iter().all(|line| !line.contains('…')));
        chrome.cancel_shortcut_recording();
        chrome.settings_search_query = "Allowed keys".into();
        let scene = WgpuCandidateRenderer::new(400.0, 1200.0).build_settings_scene(&chrome, None);
        for text in [
            "Ctrl or Alt + optional Shift",
            "A–Z / Space",
            "Click a key to record; Esc cancels",
        ] {
            let layout = scene
                .text_sections
                .iter()
                .flat_map(|section| &section.layouts)
                .find(|layout| plain(&layout.lines.join("")) == plain(language.tr(text)))
                .expect("every localized support notice is fully readable, not a truncated chip");
            assert!(!layout.truncated);
        }
    }
}

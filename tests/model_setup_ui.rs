#![cfg(feature = "gpu")]

use suzaku_map::{ime::gpu::*, ui::UiLanguage};

fn model_settings(width: f32, chrome: &PanelChromeState) -> RenderScene {
    let probe = WgpuCandidateRenderer::new(width, 2000.0).build_settings_scene(chrome, None);
    let height = probe
        .settings_scroll_metadata
        .unwrap()
        .preferred_window_height;
    WgpuCandidateRenderer::new(width, height).build_settings_scene(chrome, None)
}

#[test]
fn model_setup_guide_has_one_real_localized_pointer_and_keyboard_target() {
    for ui_language in UiLanguage::ALL {
        for width in [420.0, 620.0, 960.0] {
            let chrome = PanelChromeState {
                ui_language,
                text_scale: DisplayTextScale::Large,
                settings_category: SettingsCategory::Model,
                ..Default::default()
            };
            let scene = model_settings(width, &chrome);
            let kind = InteractionKind::OpenModelSetup;
            let targets: Vec<_> = scene
                .interactive_targets
                .iter()
                .filter(|target| target.kind == kind)
                .collect();
            assert_eq!(targets.len(), 1, "{ui_language:?}/{width}");
            assert_eq!(
                scene
                    .settings_focus_targets
                    .iter()
                    .filter(|target| target.kind == kind)
                    .count(),
                1
            );
            let rect = targets[0].rect;
            assert_eq!(
                scene.hit_interaction(rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0),
                Some(kind)
            );
            assert!(!scene.settings_option_truncated.contains(&kind));
            assert!(
                scene
                    .interactive_targets
                    .iter()
                    .all(|target| !matches!(target.kind, InteractionKind::SetLlmModel(_)))
            );
            for original in [
                InteractionKind::SetLlmEnabled(true),
                InteractionKind::SetLlmEnabled(false),
                InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Focused),
                InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Balanced),
                InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Expressive),
            ] {
                assert!(
                    scene
                        .interactive_targets
                        .iter()
                        .any(|target| target.kind == original),
                    "the guide must retain the existing model controls: {original:?}"
                );
            }
        }
    }
}

#[test]
fn model_setup_guide_is_searchable_from_other_categories_in_every_language() {
    for ui_language in UiLanguage::ALL {
        let label = ui_language.tr("Configuration path and guide");
        for search in ["Configuration path and guide", label] {
            let chrome = PanelChromeState {
                ui_language,
                settings_category: SettingsCategory::Appearance,
                settings_search_query: search.into(),
                ..Default::default()
            };
            let scene = model_settings(620.0, &chrome);
            assert!(
                scene
                    .interactive_targets
                    .iter()
                    .any(|target| target.kind == InteractionKind::OpenModelSetup),
                "{ui_language:?}: {search}"
            );
        }
        if !matches!(ui_language, UiLanguage::English | UiLanguage::System) {
            assert_ne!(label, "Configuration path and guide");
        }
    }
}

#![cfg(feature = "gpu")]

use std::{collections::HashMap, sync::Arc};
use suzaku_map::{
    ime::{XRTabletImeEngine, gpu::*},
    ui::UiLanguage,
};

fn scene(width: f32, chrome: &PanelChromeState) -> (RenderScene, f32) {
    let mut engine = XRTabletImeEngine::new(Default::default());
    let snapshot = engine.seed(&chrome.seed_text);
    let height = WgpuCandidateRenderer::new(width, 1.0).preferred_input_panel_height(chrome);
    let renderer = WgpuCandidateRenderer::new(width, height);
    assert_eq!(height, renderer.preferred_input_panel_height(chrome));
    (
        renderer.build_panel_scene(&snapshot, chrome, None, None, None, None),
        height,
    )
}

fn overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] - 0.01
        && b[0] < a[0] + a[2] - 0.01
        && a[1] < b[1] + b[3] - 0.01
        && b[1] < a[1] + a[3] - 0.01
}

fn joined(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn assert_headers(scene: &RenderScene, chrome: &PanelChromeState, width: f32, height: f32) {
    let ui = chrome.ui_language;
    let (role, title, status) = match chrome.active_input_mode {
        InputMode::VirtualKeyboard => (
            TextRole::KeyboardKey,
            "Keyboard input",
            ui.tr("Tap to type").to_string(),
        ),
        InputMode::Dictation => (
            TextRole::VoiceLabel,
            "Voice input",
            if chrome.voice_state == VoiceCaptureState::Listening {
                if chrome.voice_transcript.is_empty() {
                    ui.tr("Listening now...").to_string()
                } else {
                    ui.tr("Heard just now").to_string()
                }
            } else if chrome.voice_transcript.is_empty() {
                ui.message(&suzaku_map::platform::voice_host::voice_status_text(
                    chrome.voice_state,
                    chrome.voice_permission,
                    false,
                    &chrome.voice_backend_label,
                ))
                .into_owned()
            } else {
                ui.tr("Ready to insert").to_string()
            },
        ),
        InputMode::Handwriting => (
            TextRole::HandwritingLabel,
            "Handwrite input",
            ui.tr("Trace on canvas").to_string(),
        ),
        InputMode::Translation => unreachable!(),
    };
    let headers: Vec<_> = scene
        .text_sections
        .iter()
        .flat_map(|section| &section.layouts)
        .filter(|layout| layout.role == role)
        .take(2)
        .collect();
    assert_eq!(headers.len(), 2);
    for (header, expected) in headers.iter().zip([ui.tr(title).to_string(), status]) {
        assert!(
            !header.truncated,
            "{:?}/{width}/{:?}/{:?}: {expected}: {:?}",
            chrome.ui_language, chrome.text_scale, chrome.active_input_mode, header.lines
        );
        assert_eq!(joined(&header.lines.concat()), joined(&expected));
        assert!(header.bounds[0] >= 0.0 && header.bounds[1] >= 0.0);
        assert!(header.bounds[0] + header.bounds[2] <= width + 0.01);
        assert!(header.bounds[1] + header.bounds[3] <= height + 0.01);
        for target in &scene.interactive_targets {
            if matches!(
                target.kind,
                InteractionKind::VirtualKeyboardKey(_)
                    | InteractionKind::HandwritingCanvas
                    | InteractionKind::UndoHandwritingStroke
                    | InteractionKind::ClearHandwriting
                    | InteractionKind::ToggleVoiceCapture
                    | InteractionKind::InsertVoiceTranscript
                    | InteractionKind::CycleVoiceSample
                    | InteractionKind::OpenVoiceSettings
                    | InteractionKind::RefreshVoicePermissions
                    | InteractionKind::ClearVoiceTranscript
            ) {
                assert!(
                    !overlap(header.bounds, target.rect),
                    "header {:?} overlaps {:?} at {:?}/{width}/{:?}/{:?}",
                    header.lines,
                    target.kind,
                    chrome.ui_language,
                    chrome.text_scale,
                    chrome.active_input_mode
                );
            }
        }
    }
    assert!(!overlap(headers[0].bounds, headers[1].bounds));
    if chrome.active_input_mode == InputMode::Dictation {
        for transcript in scene
            .text_sections
            .iter()
            .flat_map(|section| &section.layouts)
            .filter(|layout| layout.role == TextRole::VoiceTranscript)
        {
            for header in &headers {
                assert!(
                    !overlap(header.bounds, transcript.bounds),
                    "header {:?} overlaps transcript {:?}",
                    header.lines,
                    transcript.lines
                );
            }
        }
    }
}

fn all_headers() {
    for ui_language in UiLanguage::ALL {
        for width in [420.0, 620.0, 960.0] {
            for text_scale in [DisplayTextScale::Medium, DisplayTextScale::Large] {
                for panel_layout_mode in [PanelLayoutMode::FollowCaret, PanelLayoutMode::BottomDock]
                {
                    for active_input_mode in [
                        InputMode::VirtualKeyboard,
                        InputMode::Dictation,
                        InputMode::Handwriting,
                    ] {
                        let chrome = PanelChromeState {
                            ui_language,
                            text_scale,
                            panel_layout_mode,
                            active_input_mode,
                            input_modes_expanded: true,
                            seed_text: "Settings".into(),
                            voice_transcript: "Theme".into(),
                            handwriting_candidates: vec!["Settings".into()],
                            ..Default::default()
                        };
                        let (scene, height) = scene(width, &chrome);
                        assert_headers(&scene, &chrome, width, height);
                        assert_eq!(chrome.seed_text, "Settings");
                        assert_eq!(chrome.voice_transcript, "Theme");
                        assert_eq!(chrome.handwriting_candidates, ["Settings"]);
                        assert!(
                            scene
                                .text_sections
                                .iter()
                                .flat_map(|section| &section.layouts)
                                .any(|layout| layout.role == TextRole::InputValue
                                    && layout.lines == ["Settings"])
                        );
                        if active_input_mode == InputMode::Dictation {
                            assert!(
                                scene
                                    .text_sections
                                    .iter()
                                    .flat_map(|section| &section.layouts)
                                    .any(|layout| layout.role == TextRole::VoiceTranscript
                                        && layout.lines == ["Theme"])
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn tool_headers_reflow_in_eight_languages_without_covering_input_controls() {
    all_headers();
}

#[test]
fn natural_height_and_tool_headers_share_the_active_font_measurements() {
    let mut advances = HashMap::new();
    for language in UiLanguage::ALL {
        for key in [
            "Keyboard input",
            "Tap to type",
            "Voice input",
            "Ready to insert",
            "Handwrite input",
            "Trace on canvas",
        ] {
            for ch in language.tr(key).chars() {
                advances.insert(
                    ch,
                    if ch.is_whitespace() {
                        2.5
                    } else if ch.is_ascii_uppercase() {
                        6.2
                    } else {
                        5.3
                    },
                );
            }
        }
    }
    with_font_metrics(Arc::new(advances), all_headers);
}

#[test]
fn listening_labels_reflow_without_changing_the_raw_transcript() {
    for ui_language in UiLanguage::ALL {
        for width in [420.0, 620.0, 960.0] {
            for text_scale in [DisplayTextScale::Medium, DisplayTextScale::Large] {
                for transcript in ["", "Settings"] {
                    let chrome = PanelChromeState {
                        ui_language,
                        text_scale,
                        input_modes_expanded: true,
                        active_input_mode: InputMode::Dictation,
                        voice_state: VoiceCaptureState::Listening,
                        voice_transcript: transcript.into(),
                        ..Default::default()
                    };
                    let (scene, height) = scene(width, &chrome);
                    assert_headers(&scene, &chrome, width, height);
                    assert_eq!(chrome.voice_transcript, transcript);
                }
            }
        }
    }
}

#[test]
fn empty_voice_states_keep_program_messages_and_backend_parameters_separate() {
    for ui_language in UiLanguage::ALL {
        for width in [420.0, 620.0, 960.0] {
            for text_scale in [DisplayTextScale::Medium, DisplayTextScale::Large] {
                for voice_permission in [
                    VoicePermissionState::Unknown,
                    VoicePermissionState::Ready,
                    VoicePermissionState::Pending,
                    VoicePermissionState::Denied,
                    VoicePermissionState::Error,
                    VoicePermissionState::Unavailable,
                ] {
                    for backend in [
                        "Settings",
                        "model://Settings/Theme/Llama-3.2-provider-long-context-language-id-0123456789",
                    ] {
                        for voice_supports_live_capture in [false, true] {
                            let chrome = PanelChromeState {
                                ui_language,
                                text_scale,
                                voice_permission,
                                voice_supports_live_capture,
                                input_modes_expanded: true,
                                active_input_mode: InputMode::Dictation,
                                voice_state: VoiceCaptureState::Idle,
                                voice_backend_label: backend.into(),
                                ..Default::default()
                            };
                            let (scene, height) = scene(width, &chrome);
                            assert_headers(&scene, &chrome, width, height);
                            let status = scene
                                .text_sections
                                .iter()
                                .flat_map(|section| &section.layouts)
                                .filter(|layout| layout.role == TextRole::VoiceLabel)
                                .nth(1)
                                .unwrap();
                            if voice_permission != VoicePermissionState::Denied {
                                assert!(
                                    joined(&status.lines.concat()).contains(&joined(backend)),
                                    "backend identity was translated: {ui_language:?}/{voice_permission:?}: {:?}",
                                    status.lines
                                );
                            }
                            assert!(chrome.voice_transcript.is_empty());
                            assert_eq!(chrome.voice_backend_label, backend);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn oversized_external_voice_backend_is_bounded_after_the_localized_status_prefix() {
    for ui_language in UiLanguage::ALL {
        for voice_permission in [
            VoicePermissionState::Unknown,
            VoicePermissionState::Pending,
            VoicePermissionState::Error,
        ] {
            let chrome = PanelChromeState {
                ui_language,
                voice_permission,
                input_modes_expanded: true,
                active_input_mode: InputMode::Dictation,
                text_scale: DisplayTextScale::Large,
                voice_backend_label: "model://Settings/Theme/".repeat(200),
                ..Default::default()
            };
            let (scene, height) = scene(420.0, &chrome);
            let headers: Vec<_> = scene
                .text_sections
                .iter()
                .flat_map(|section| &section.layouts)
                .filter(|layout| layout.role == TextRole::VoiceLabel)
                .take(2)
                .collect();
            assert!(!headers[0].truncated);
            assert_eq!(headers[0].lines, [ui_language.tr("Voice input")]);
            assert!(headers[1].truncated);
            assert!(headers[1].lines.len() <= 4);
            let source = suzaku_map::platform::voice_host::voice_status_text(
                VoiceCaptureState::Idle,
                voice_permission,
                false,
                "BACKEND_SENTINEL",
            );
            let localized = ui_language.message(&source);
            let prefix = localized.split_once("BACKEND_SENTINEL").unwrap().0;
            assert!(joined(&headers[1].lines.concat()).starts_with(&joined(prefix)));
            assert!(!overlap(headers[0].bounds, headers[1].bounds));
            for header in headers {
                assert!(header.bounds[0] >= 0.0 && header.bounds[1] >= 0.0);
                assert!(header.bounds[0] + header.bounds[2] <= 420.01);
                assert!(header.bounds[1] + header.bounds[3] <= height + 0.01);
                for transcript in scene
                    .text_sections
                    .iter()
                    .flat_map(|section| &section.layouts)
                    .filter(|layout| layout.role == TextRole::VoiceTranscript)
                {
                    assert!(!overlap(header.bounds, transcript.bounds));
                }
            }
        }
    }
}

#[test]
fn pending_tool_resize_keeps_headers_and_controls_inside_the_old_viewport() {
    let mut engine = XRTabletImeEngine::new(Default::default());
    let snapshot = engine.seed("");
    for (ui_language, active_input_mode, voice_permission) in [
        (
            UiLanguage::German,
            InputMode::VirtualKeyboard,
            VoicePermissionState::Unknown,
        ),
        (
            UiLanguage::English,
            InputMode::Dictation,
            VoicePermissionState::Pending,
        ),
    ] {
        let chrome = PanelChromeState {
            ui_language,
            active_input_mode,
            voice_permission,
            input_modes_expanded: true,
            text_scale: DisplayTextScale::Large,
            voice_backend_label:
                "model://Settings/Theme/Llama-3.2-provider-long-context-language-id-0123456789"
                    .into(),
            ..Default::default()
        };
        for height in [180.0, 220.0, 260.0] {
            let scene = WgpuCandidateRenderer::new(420.0, height)
                .build_panel_scene(&snapshot, &chrome, None, None, None, None);
            let role = if active_input_mode == InputMode::VirtualKeyboard {
                TextRole::KeyboardKey
            } else {
                TextRole::VoiceLabel
            };
            for header in scene
                .text_sections
                .iter()
                .flat_map(|section| &section.layouts)
                .filter(|layout| layout.role == role)
                .take(2)
            {
                assert!(
                    header.bounds[1] + header.bounds[3] <= height + 0.01,
                    "{ui_language:?}/{active_input_mode:?}/{height}: header {:?} escapes old viewport: {:?}",
                    header.lines,
                    header.bounds
                );
            }
            for target in &scene.interactive_targets {
                if matches!(
                    target.kind,
                    InteractionKind::VirtualKeyboardKey(_)
                        | InteractionKind::ToggleVoiceCapture
                        | InteractionKind::InsertVoiceTranscript
                        | InteractionKind::CycleVoiceSample
                        | InteractionKind::OpenVoiceSettings
                        | InteractionKind::RefreshVoicePermissions
                        | InteractionKind::ClearVoiceTranscript
                ) {
                    assert!(target.rect[0] >= 0.0 && target.rect[1] >= 0.0);
                    assert!(target.rect[0] + target.rect[2] <= 420.01);
                    assert!(
                        target.rect[1] + target.rect[3] <= height + 0.01,
                        "{ui_language:?}/{active_input_mode:?}/{height}: {:?} escapes old viewport: {:?}",
                        target.kind,
                        target.rect
                    );
                }
            }
        }
    }
}

#![cfg(feature = "gpu")]

use suzaku_map::{
    ime::{XRTabletImeEngine, gpu::*},
    panel_support::sentence_candidate_style_label,
    ui::UiLanguage,
};

const STYLE_KEYS: [&str; 9] = [
    "Typed",
    "Best",
    "Short",
    "Guided",
    "Expanded",
    "Natural",
    "Polite",
    "Detailed",
    "Alternate",
];

fn scene(
    ui_language: UiLanguage,
    seed: &str,
    candidate: &str,
    preview_style: PreviewStyle,
    width: f32,
    text_scale: DisplayTextScale,
) -> RenderScene {
    let mut engine = XRTabletImeEngine::new(Default::default());
    let mut snapshot = engine.seed(seed);
    // Exercise the unselected candidate description, not the selection-status override.
    snapshot.selected_index = usize::MAX;
    let chrome = PanelChromeState {
        ui_language,
        seed_text: seed.into(),
        input_modes_expanded: true,
        preview_style,
        text_scale,
        sentence_candidates: vec![candidate.into()],
        sentence_candidate_source_indices: vec![7],
        ..Default::default()
    };
    WgpuCandidateRenderer::new(width, 2000.0)
        .build_panel_scene(&snapshot, &chrome, None, None, None, None)
}

fn interface_languages() -> impl Iterator<Item = UiLanguage> {
    // The eight explicit choices are independent of the test runner's locale.
    UiLanguage::ALL
        .into_iter()
        .filter(|language| *language != UiLanguage::System)
}

#[test]
fn candidate_style_keys_have_complete_bundled_resources() {
    let rows = include_str!("../src/ui/catalog.tsv")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for key in STYLE_KEYS {
        let matches = rows.iter().filter(|row| row[0] == key).collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "one bundled resource for {key}");
        assert_eq!(matches[0].len(), 8, "seven translations for {key}");
        assert!(matches[0].iter().all(|field| !field.is_empty()));
    }
}

#[test]
fn candidate_style_descriptions_translate_program_owned_parameters_in_eight_interfaces() {
    let cases = [
        ("a b", "Short"),
        ("a b c d next suggestion", "Guided"),
        ("a b c d complete sentence", "Expanded"),
        ("a b c d is ready.", "Natural"),
        ("a b c d e please", "Polite"),
        ("a b c d e f g h i j k", "Detailed"),
        ("a b c d e f", "Alternate"),
    ];
    for ui in interface_languages() {
        for (candidate, key) in cases {
            assert_eq!(sentence_candidate_style_label(candidate, false), key);
            let expected = ui
                .tr("sentence {} phrase · tap to commit")
                .replace("{}", ui.tr(key));
            for width in [720.0, 1200.0] {
                let scene = scene(
                    ui,
                    "unrelated draft",
                    candidate,
                    PreviewStyle::Full,
                    width,
                    DisplayTextScale::Medium,
                );
                let metadata = scene
                    .text_sections
                    .iter()
                    .flat_map(|section| &section.layouts)
                    .filter(|layout| layout.role == TextRole::CandidateMeta)
                    .collect::<Vec<_>>();
                assert!(
                    metadata
                        .iter()
                        .any(|layout| layout.lines == [expected.as_str()] && !layout.truncated),
                    "{ui:?}/{key}/{width}: expected {expected:?}, got {metadata:?}"
                );
            }
        }
    }
}

#[test]
fn hero_badges_translate_and_fit_in_eight_interfaces() {
    for ui in interface_languages() {
        for (seed, candidate, key) in [
            ("hello", "hello", "Typed"),
            ("hello", "hello world", "Best"),
        ] {
            for width in [420.0, 720.0, 1200.0] {
                for text_scale in [DisplayTextScale::Medium, DisplayTextScale::Large] {
                    let scene = scene(
                        ui,
                        seed,
                        candidate,
                        PreviewStyle::Compact,
                        width,
                        text_scale,
                    );
                    let badge = scene
                        .text_sections
                        .iter()
                        .find(|section| section.role == TextRole::CandidateMeta)
                        .and_then(|section| section.layouts.first())
                        .expect("hero candidate style badge");
                    assert_eq!(
                        badge.lines,
                        [ui.tr(key)],
                        "{ui:?}/{key}/{width}/{text_scale:?}"
                    );
                    assert!(
                        !badge.truncated,
                        "{ui:?}/{key}/{width}/{text_scale:?}: {badge:?}"
                    );
                    assert!(badge.bounds[0] >= 0.0);
                    assert!(badge.bounds[0] + badge.bounds[2] <= width);
                }
            }
        }
    }
}

#[test]
fn candidate_contents_matching_style_keys_remain_original_in_all_interfaces() {
    for ui in interface_languages() {
        for candidate in STYLE_KEYS {
            // Deliberately use the literal candidate as the draft too. Its badge is
            // application chrome (Typed), but neither payload may be translated.
            let scene = scene(
                ui,
                candidate,
                candidate,
                PreviewStyle::Compact,
                960.0,
                DisplayTextScale::Large,
            );
            for role in [TextRole::InputValue, TextRole::CandidatePrimary] {
                let layouts = scene
                    .text_sections
                    .iter()
                    .flat_map(|section| &section.layouts)
                    .filter(|layout| layout.role == role)
                    .collect::<Vec<_>>();
                assert!(
                    layouts
                        .iter()
                        .any(|layout| layout.lines == [candidate] && !layout.truncated),
                    "{ui:?}/{candidate}/{role:?}: {layouts:?}"
                );
            }
            assert!(
                scene
                    .text_sections
                    .iter()
                    .filter(|section| section.role == TextRole::CandidateMeta)
                    .flat_map(|section| &section.layouts)
                    .any(|layout| layout.lines == [ui.tr("Typed")])
            );
            assert_eq!(
                scene
                    .interactive_targets
                    .iter()
                    .filter_map(|target| match target.kind {
                        InteractionKind::Candidate(index) => Some(index),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                [7],
                "localization must preserve the candidate's source index"
            );
        }
    }
}

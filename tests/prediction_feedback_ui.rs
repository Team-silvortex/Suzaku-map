#![cfg(feature = "gpu")]
use suzaku_map::{
    ime::{EngineConfig, XRTabletImeEngine, gpu::*},
    prediction::PredictionError,
    ui::{PredictionNotice, UiLanguage},
};

fn scene(chrome: &PanelChromeState, width: f32) -> RenderScene {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.set_preferences_enabled(false);
    engine.seed("hel");
    WgpuCandidateRenderer::new(width, 600.0).build_panel_scene(
        &engine.snapshot(),
        chrome,
        None,
        None,
        None,
        None,
    )
}

#[test]
fn notices_replace_only_the_existing_single_line_header_without_reflow_or_new_targets() {
    for ui_language in UiLanguage::ALL
        .into_iter()
        .filter(|ui| *ui != UiLanguage::System)
    {
        for width in [420.0, 650.0, 900.0] {
            for text_scale in [
                DisplayTextScale::Small,
                DisplayTextScale::Medium,
                DisplayTextScale::Large,
            ] {
                let mut chrome = PanelChromeState {
                    ui_language,
                    text_scale,
                    seed_text: "hel".into(),
                    ..Default::default()
                };
                let before = scene(&chrome, width);
                for notice in [
                    PredictionNotice::Waiting,
                    PredictionNotice::Received,
                    PredictionNotice::Error(PredictionError::Timeout),
                    PredictionNotice::Error(PredictionError::MissingCredentials),
                    PredictionNotice::Error(PredictionError::CloudConsentRequired),
                ] {
                    chrome.prediction_notice = Some(notice.clone());
                    let after = scene(&chrome, width);
                    assert_eq!(after.quads, before.quads);
                    assert_eq!(after.interactive_targets, before.interactive_targets);
                    assert_eq!(after.hit_targets, before.hit_targets);
                    assert_eq!(after.labels, before.labels);
                    assert_eq!(after.draft_text, before.draft_text);
                    let header = after
                        .text_sections
                        .iter()
                        .find(|section| section.role == TextRole::InputLabel)
                        .unwrap();
                    let baseline = before
                        .text_sections
                        .iter()
                        .find(|section| section.role == TextRole::InputLabel)
                        .unwrap();
                    assert_eq!(header.layouts.len(), baseline.layouts.len());
                    // The second layout is the editable user draft, not chrome text.
                    assert_eq!(header.layouts[1], baseline.layouts[1]);
                    let title = &header.layouts[0];
                    assert!(title.lines.len() <= 1);
                    assert_eq!(title.bounds[0], baseline.layouts[0].bounds[0]);
                    assert_eq!(title.bounds[1], baseline.layouts[0].bounds[1]);
                    assert!(title.bounds[0] >= 0.0 && title.bounds[0] + title.bounds[2] <= width);
                    let expected = notice.render(ui_language);
                    if !title.truncated {
                        assert_eq!(title.lines.join(""), expected);
                    }
                }
            }
        }
    }
}

#[test]
fn notice_is_not_rendered_in_settings_or_compact_scenes() {
    let baseline = PanelChromeState::default();
    let with_notice = PanelChromeState {
        prediction_notice: Some(PredictionNotice::Error(PredictionError::MissingCredentials)),
        ..baseline.clone()
    };
    let renderer = WgpuCandidateRenderer::new(900.0, 600.0);
    assert_eq!(
        renderer.build_settings_scene(&baseline, None),
        renderer.build_settings_scene(&with_notice, None)
    );
    assert_eq!(
        scene(
            &PanelChromeState {
                compact_mode: true,
                ..baseline
            },
            900.0
        ),
        scene(
            &PanelChromeState {
                compact_mode: true,
                ..with_notice
            },
            900.0
        )
    );
}

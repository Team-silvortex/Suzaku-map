//! Repeated engine controls are not new input or an explicit candidate choice.
//! Fixed offline drafts only; no native keys, personal settings or live model.
use suzaku_map::ime::candidate_mix::CandidateKind;
use suzaku_map::ime::{
    Candidate, CommitOptions, EngineConfig, LanguagePlugin, PredictionStatus, SignalState,
    XRTabletImeEngine,
};

#[derive(Clone, Copy, Debug)]
enum RepeatedControl {
    Language,
    Signal,
}

fn check_explicit_choices(control: RepeatedControl) {
    let long_en = "Earlier café 😀.  ".repeat(20);
    let long_zh = "前文已确认。".repeat(60);
    let cases = [
        ("en", "en_GB", "", "hel", "hello", "hello, how are you?"),
        (
            "zh-Hans",
            "ZH_cn",
            "",
            "ni hao",
            "你好",
            "你好，很高兴认识你。",
        ),
        (
            "ja",
            "ja_JP",
            "",
            "sumimasen",
            "すみません",
            "すみません、もう一度お願いします。",
        ),
        (
            "en",
            "en_AU",
            long_en.as_str(),
            "please sen",
            "please send",
            "please send me the details.",
        ),
        (
            "zh-Hans",
            "zh_SG",
            long_zh.as_str(),
            "ni hao",
            "你好",
            "你好，很高兴认识你。",
        ),
    ];
    let mut checked = 0;
    for (language, alias, prefix, raw, word, sentence) in cases {
        let seed = format!("{prefix}{raw}");
        for mixed in [false, true] {
            // The short non-IBus API supplies words, not the mixed sentence pool.
            let choices = if mixed || seed.chars().count() > 256 {
                vec![
                    (word, CandidateKind::Word),
                    (sentence, CandidateKind::Sentence),
                ]
            } else {
                vec![(word, CandidateKind::Unspecified)]
            };
            for (choice, kind) in choices {
                let mut engine = XRTabletImeEngine::new(EngineConfig {
                    default_language: language.into(),
                    ..Default::default()
                });
                // Apply once before selecting: the first signal application
                // still initializes confidence from the startup defaults.
                engine.update_signal(SignalState::default());
                if mixed {
                    engine.enable_ibus_candidate_mix();
                }
                engine.seed(&seed);
                let text = format!("{prefix}{choice}");
                let index = engine
                    .candidates()
                    .iter()
                    .position(|candidate| candidate.text == text)
                    .unwrap_or_else(|| panic!("missing {language}/{mixed} choice {text:?}"));
                if mixed || seed.chars().count() > 256 {
                    assert_eq!(engine.candidates()[index].kind, kind);
                }
                engine.select_candidate(index);
                let before = engine.snapshot();
                let pool = engine.candidates().to_vec();
                for _ in 0..2 {
                    let after = match control {
                        RepeatedControl::Language => engine.set_language(alias),
                        RepeatedControl::Signal => engine.update_signal(SignalState::default()),
                    };
                    assert_eq!(after, before, "{control:?}/{language}/{mixed}/{kind:?}");
                    assert_eq!(engine.candidates(), pool);
                    assert_eq!(engine.selected_completion_text(true), Some(text.as_str()));
                    assert_eq!(engine.prediction_status(), PredictionStatus::Disabled);
                    assert_eq!(engine.preference_count(), 0);
                }
                let result = engine.commit(CommitOptions { force: true });
                assert!(result.ok);
                assert_eq!(result.text.as_deref(), Some(text.as_str()));
                assert!(engine.undo().unwrap().committed_text.is_empty());
                assert!(engine.undo().is_none());
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 17);
}

#[test]
fn repeated_language_aliases_preserve_explicit_word_and_sentence_choices() {
    check_explicit_choices(RepeatedControl::Language);
}

#[test]
fn repeated_signal_preserves_explicit_word_and_sentence_choices() {
    check_explicit_choices(RepeatedControl::Signal);
}

#[test]
fn initial_and_changed_signals_and_real_language_changes_still_rebuild() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    assert_eq!(engine.snapshot().confidence, 1.0);
    let initialized = engine.update_signal(SignalState::default());
    assert!((initialized.confidence - 0.9).abs() < 0.000_001);
    engine.enable_ibus_candidate_mix();
    engine.seed("ni hao");
    let old = engine.candidates().to_vec();
    let converted = engine.set_language("zh_CN");
    assert_eq!(converted.active_language, "zh-Hans");
    assert_eq!(converted.seed_text, "ni hao");
    assert_eq!(engine.candidates()[0].text, "你好");
    assert_ne!(engine.candidates(), old);
    assert_eq!(engine.selected_completion_text(true), None);

    engine.select_candidate(0);
    let degraded = engine.update_signal(SignalState {
        pointer_precision: 0.2,
        gaze_stability: 0.2,
        host_intent_weight: 0.2,
        source_confidence: 0.2,
    });
    assert!(degraded.degraded);
    assert!(engine.candidates().len() <= 3);
    assert_eq!(engine.selected_completion_text(true), None);
    let pool = engine.candidates().to_vec();
    assert_eq!(
        engine.update_signal(SignalState {
            pointer_precision: 0.2,
            gaze_stability: 0.2,
            host_intent_weight: 0.2,
            source_confidence: 0.2,
        }),
        degraded
    );
    assert_eq!(engine.candidates(), pool);
}

struct ReplacementPlugin(&'static str);

impl LanguagePlugin for ReplacementPlugin {
    fn id(&self) -> &str {
        self.0
    }

    fn normalize_seed(&self, input: &str) -> String {
        input.into()
    }

    fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
        vec![token.into()]
    }

    fn build_candidates(&self, _parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        vec![Candidate {
            text: format!("replacement: {seed}"),
            label: "replacement plugin".into(),
            score: confidence + 1.0,
            ..Default::default()
        }]
    }
}

#[test]
fn replacing_the_active_plugin_is_not_mistaken_for_a_repeated_control() {
    for control in [RepeatedControl::Language, RepeatedControl::Signal] {
        let mut engine = XRTabletImeEngine::new(EngineConfig::default());
        engine.update_signal(SignalState::default());
        engine.seed("hel");
        let old = engine.candidates().to_vec();
        engine.register_language_plugin(ReplacementPlugin("en"));
        // Registration has always been lazy; preserve that API contract.
        assert_eq!(engine.candidates(), old);
        match control {
            RepeatedControl::Language => engine.set_language("en_US"),
            RepeatedControl::Signal => engine.update_signal(SignalState::default()),
        };
        assert_eq!(engine.candidates()[0].text, "replacement: hel");
        engine.select_candidate(0);
        let updated = engine.snapshot();
        match control {
            RepeatedControl::Language => engine.set_language("en_GB"),
            RepeatedControl::Signal => engine.update_signal(SignalState::default()),
        };
        assert_eq!(engine.snapshot(), updated);
        assert_eq!(
            engine.selected_completion_text(true),
            Some("replacement: hel")
        );
    }
}

#[test]
fn registering_an_inactive_plugin_does_not_invalidate_the_current_choice() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.update_signal(SignalState::default());
    engine.seed("hel");
    engine.select_candidate(1);
    let text = engine.selected_completion_text(true).unwrap().to_owned();
    let before = engine.snapshot();
    engine.register_language_plugin(ReplacementPlugin("fixture"));
    assert_eq!(engine.set_language("en"), before);
    assert_eq!(engine.selected_completion_text(true), Some(text.as_str()));
    assert_eq!(engine.update_signal(SignalState::default()), before);
    engine.set_language("fixture");
    assert_eq!(engine.candidates()[0].text, "replacement: hel");
}

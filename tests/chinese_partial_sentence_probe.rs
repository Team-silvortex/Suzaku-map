//! Fixed regressions from existing vocabulary, not new words or page-one quotas.
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource};
use suzaku_map::ime::{EngineConfig, XRTabletImeEngine};

fn assert_existing_sentence(seed: &str, word: &str, sentence: &str) {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .any(|choice| { choice.text == seed && choice.kind == CandidateKind::Literal })
    );
    assert!(
        ime.candidates()
            .iter()
            .any(|choice| { choice.text == word && choice.kind == CandidateKind::Word }),
        "missing completed Word: {:?}",
        ime.candidates()
    );
    assert!(
        ime.candidates().iter().any(|choice| {
            choice.text == sentence
                && choice.kind == CandidateKind::Sentence
                && choice.source == CandidateSource::Local
        }),
        "missing authored Sentence: {:?}",
        ime.candidates()
    );
}

#[test]
fn existing_coupon_sentence_survives_a_partial_reading_after_adopted_han() {
    assert_existing_sentence(
        "优惠券还在youxi",
        "优惠券还在有效期",
        "优惠券还在有效期内。",
    );
}

#[test]
fn existing_retry_sentence_survives_a_partial_reading_after_adopted_han() {
    assert_existing_sentence(
        "登录失败后可以zaishiyi",
        "登录失败后可以再试一次",
        "登录失败后可以再试一次。",
    );
}

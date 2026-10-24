//! The one historical replay inverse must not weaken current candidate behavior.
#[path = "support/daily_entryway_replay.rs"]
mod daily_entryway_replay;

use suzaku_map::ime::{
    EngineConfig, XRTabletImeEngine,
    candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
};

fn current() -> Vec<suzaku_map::ime::Candidate> {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed("kutsu");
    assert_eq!(ime.snapshot().seed_text, "kutsu");
    assert!(ime.snapshot().committed_text.is_empty());
    ime.candidates().to_vec()
}

#[test]
fn entryway_replay_checks_actual_twelve_and_restores_real_ten_without_mutating_either() {
    let current = current();
    let saved = current.clone();
    assert_eq!(current.len(), 12);
    for (text, score) in [("靴箱", 89.5_f32), ("靴べら", 89.0)] {
        let candidate = current.iter().find(|c| c.text == text).unwrap();
        assert_eq!(candidate.label, text);
        assert_eq!(candidate.kind, CandidateKind::Word);
        assert_eq!(candidate.source, CandidateSource::Local);
        assert_eq!(candidate.score.to_bits(), score.to_bits());
    }
    // Both old root authors still occupy the real first page.
    for author in [
        "靴を履いたら、すぐに出ます。",
        "靴は歩きやすいものを選びます。",
    ] {
        assert!(
            current
                .iter()
                .position(|c| c.text == author && c.kind == CandidateKind::Sentence)
                .unwrap()
                < PAGE_SIZE
        );
    }
    let old = daily_entryway_replay::before_daily_entryway("ja", "kutsu", &current, false);
    assert_eq!(old.len(), 10);
    assert!(
        old.iter()
            .all(|c| !matches!(c.text.as_str(), "靴箱" | "靴べら"))
    );
    assert_eq!(
        current, saved,
        "projection must not mutate the actual runtime frame"
    );
}

#[test]
fn entryway_replay_passes_other_scopes_and_rejects_every_tampered_candidate_field() {
    let current = current();
    for (language, seed, packs) in [
        ("en", "kutsu", false),
        ("ja", "ma", false),
        ("ja", "kutsu", true),
    ] {
        assert_eq!(
            daily_entryway_replay::before_daily_entryway(language, seed, &current, packs),
            current
        );
    }
    let mut changed = Vec::new();
    let mut frame = current.clone();
    frame[0].text.push('!');
    changed.push(frame);
    let mut frame = current.clone();
    frame[0].label.push('!');
    changed.push(frame);
    let mut frame = current.clone();
    frame[0].kind = CandidateKind::Unspecified;
    changed.push(frame);
    let mut frame = current.clone();
    frame[0].source = CandidateSource::Model;
    changed.push(frame);
    let mut frame = current.clone();
    frame[0].score = f32::from_bits(frame[0].score.to_bits() - 1);
    changed.push(frame);
    let mut frame = current.clone();
    frame.remove(7);
    changed.push(frame);
    let mut frame = current.clone();
    frame.push(current[0].clone());
    changed.push(frame);
    let mut frame = current.clone();
    frame.swap(1, 2);
    changed.push(frame);
    let mut frame = current.clone();
    frame[7].text = "靴".into();
    frame[7].label = "靴".into();
    changed.push(frame);
    let mut frame = current.clone();
    frame[4].score += 1.0;
    changed.push(frame);
    for frame in changed {
        let outcome = std::panic::catch_unwind(|| {
            daily_entryway_replay::before_daily_entryway("ja", "kutsu", &frame, false)
        });
        assert!(
            outcome.is_err(),
            "no unknown delta may be silently removed or reweighted"
        );
    }
}

//! Real pre-append vectors include the previous unreleased messaging batch.
//! Booking-related pools may gain useful choices; existing entry points must
//! retain their old primary, authored pairs, exact raw choice and explicit undo.
use suzaku_map::ime::{
    CommitOptions, EngineConfig, XRTabletImeEngine,
    candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
};

fn engine(language: &str, seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
    );
    ime
}

fn unchanged(language: &str, cases: &[(&str, u64)]) {
    for &(seed, expected) in cases {
        let ime = engine(language, seed);
        let mut hash = 0xcbf29ce484222325_u64;
        for c in ime.candidates() {
            for byte in c
                .text
                .bytes()
                .chain([0])
                .chain(c.label.bytes())
                .chain([0])
                .chain(c.kind.id().bytes())
                .chain([0])
                .chain(c.source.id().bytes())
                .chain([0])
                .chain(c.score.to_bits().to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        assert_eq!(
            hash,
            expected,
            "{language} {seed:?}: {:?}",
            ime.candidates()
        );
    }
}

fn commit_once(language: &str, seed: &str, text: &str, kind: CandidateKind, first_page: bool) {
    let mut ime = engine(language, seed);
    let index = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "{seed:?}: missing {kind:?} {text:?}: {:?}",
                ime.candidates()
            )
        });
    if first_page {
        assert!(index < PAGE_SIZE, "{seed:?}: {text:?} at {index}");
    }
    ime.select_candidate(index);
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert!(ime.candidates().is_empty());
    assert!(!ime.commit(CommitOptions { force: true }).ok);
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn appointments_preserve_nine_old_english_complete_vectors() {
    unchanged(
        "en",
        &[
            ("book", 0x1de90e2d6d58a0c3),
            ("reservation", 0x0ca1d5692234f1e1),
            ("appointment", 0x897657b0db5d3203),
            ("opening", 0xd37eaa5bb961d494),
            ("queue", 0xc33d61eff64b7a2d),
            ("i need to make a reservation", 0x341faf2706f6fc45),
            ("can i book an appointment", 0x09ae0a1b7b09700a),
            ("could you check the opening hours", 0x8e23461f049af54c),
            ("i only just saw your messag", 0x014f05d776df08da),
        ],
    );
}

#[test]
fn appointments_preserve_four_chinese_vectors_and_original_booking_authors() {
    unchanged(
        "zh-Hans",
        &[
            ("gang kan dao ni de xiao xi", 0x18d7d7fbeda36e81),
            ("deng", 0x69a8f8c6352dc0bd),
            ("shi jian", 0x5041fca88ae63128),
            ("pai dui deng hou", 0xb01a9e5af8399d0b),
        ],
    );
    for seed in ["yuyue", "yu yue", "YU'YUE"] {
        let ime = engine("zh-Hans", seed);
        assert_eq!(ime.candidates()[0].text, "预约");
        assert_eq!(ime.candidates()[0].score.to_bits(), 100.0_f32.to_bits());
        let old = ["预约", "预约时间", "预约维修", "预约借阅"];
        let retained: Vec<_> = ime
            .candidates()
            .iter()
            .filter(|c| old.contains(&c.text.as_str()))
            .map(|c| c.text.as_str())
            .collect();
        assert_eq!(retained, old);
        commit_once("zh-Hans", seed, "预约", CandidateKind::Word, true);
        for text in ["预约已经确认了。", "预约时间可以修改吗？"] {
            commit_once("zh-Hans", seed, text, CandidateKind::Sentence, true);
        }
        for text in ["预约维修的时间已经确认了。", "预约维修需要提前联系。"]
        {
            commit_once("zh-Hans", seed, text, CandidateKind::Sentence, false);
        }
        commit_once("zh-Hans", seed, seed, CandidateKind::Literal, false);
    }
    // This pool may gain new business-hour words. Keep its old actual authors,
    // rather than refreshing its complete fingerprint to hide displaced text.
    for text in ["营业时间是几点到几点？", "营业时间需要提前确认一下。"] {
        commit_once("zh-Hans", "ying ye", text, CandidateKind::Sentence, true);
    }
}

#[test]
fn appointments_preserve_four_japanese_vectors_and_original_reception_words() {
    unchanged(
        "ja",
        &[
            ("jikan", 0x48bce107fcee5025),
            ("hoteru", 0x94f3192c31342a01),
            ("renraku", 0xd1d7ac502fc53ead),
            ("messe-ji", 0x0f200bfa259678d4),
        ],
    );
    for (seed, primary) in [
        ("yoyaku", "予約"),
        ("uketsuke", "受付"),
        ("uketsukebangou", "受付番号"),
    ] {
        let ime = engine("ja", seed);
        assert_eq!(ime.candidates()[0].text, primary);
        assert_eq!(ime.candidates()[0].score.to_bits(), 100.0_f32.to_bits());
        commit_once("ja", seed, primary, CandidateKind::Word, true);
        commit_once("ja", seed, seed, CandidateKind::Literal, false);
    }
    commit_once("ja", "uketsuke", "受付番号", CandidateKind::Word, true);
}

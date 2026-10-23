//! Frozen 6edfa3e before-data vectors; known new root extensions are tested separately.
use suzaku_map::ime::{
    CommitOptions, EngineConfig, XRTabletImeEngine,
    candidate_mix::{CandidateKind, CandidateSource},
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
    assert!(ime.candidates().len() <= 12);
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

fn old_adoption(language: &str, seed: &str, word: &str, sentence: &str) {
    for (text, kind) in [
        (word, CandidateKind::Word),
        (sentence, CandidateKind::Sentence),
    ] {
        let mut ime = engine(language, seed);
        let index = ime
            .candidates()
            .iter()
            .position(|c| c.text == text && c.kind == kind)
            .unwrap_or_else(|| panic!("missing old {text:?}: {:?}", ime.candidates()));
        assert!(index < 6, "old first-page choice moved");
        ime.select_candidate(index);
        assert_eq!(ime.selected_completion_text(true), Some(text));
        assert_eq!(ime.snapshot().seed_text, seed);
        assert!(ime.snapshot().committed_text.is_empty());
        let committed = ime.commit(CommitOptions { force: true });
        assert!(committed.ok);
        assert_eq!(committed.text.as_deref(), Some(text));
        assert_eq!(ime.snapshot().committed_text, text);
        assert!(ime.candidates().is_empty());
        assert!(!ime.commit(CommitOptions { force: true }).ok);
        assert!(ime.undo().unwrap().committed_text.is_empty());
        assert!(ime.undo().is_none());
    }
}

#[test]
fn dressing_preserves_8_actual_english_pools_and_existing_adoption() {
    unchanged(
        "en",
        &[
            ("please put on your", 0x0452725ffc45e793),
            ("i need to bring my glass", 0xf5185e0381b457a3),
            ("this shirt is still", 0xee37107e437c2067),
            ("these socks", 0x0ddb8af2778a9984),
            ("please check your pockets", 0xfd0b242d0af69413),
            ("i found your glass", 0xaf63ee0bd377ef67),
            ("please check whether ", 0xb284085053f9fd6f),
            ("we've run out of toothpast", 0x39c01d8578dda304),
        ],
    );
    old_adoption(
        "en",
        "please put on your",
        "please put on your jacket",
        "please put on your jacket before we leave.",
    );
}

#[test]
fn dressing_preserves_6_actual_chinese_pools_and_existing_adoption() {
    unchanged(
        "zh-Hans",
        &[
            ("ba la lian la shang", 0x9ed1fe93471a92fb),
            ("xie dai xi hao le", 0xbdba30a29df0681a),
            ("xiu zi you dian chang", 0x59370a8b7a3ce67d),
            ("kou dai", 0x18073ce829a44656),
            ("shi chuan", 0x036e4473f63a5948),
            ("ya gao", 0xe998d0558d7afe2f),
        ],
    );
    old_adoption(
        "zh-Hans",
        "ba la lian la shang",
        "把拉链拉上",
        "把拉链拉上再出门，外面风有点大。",
    );
}

#[test]
fn dressing_preserves_7_actual_japanese_pools_and_existing_adoption() {
    unchanged(
        "ja",
        &[
            ("uwagi", 0x7769ad7f804cc073),
            ("kigae", 0xa1dd1fc5b58ae089),
            ("sengan", 0xbbbc6057af6ea136),
            ("shanpu-", 0xf77c3a7f15a7ed5c),
            ("senganryo", 0x0f67ed4192350e0a),
            ("yoyakubangou", 0xebaa8c04564da791),
            ("nihongo wo be", 0xef8a1ca086c67568),
        ],
    );
    old_adoption(
        "ja",
        "uwagi",
        "上着",
        "上着を持っていくので、少し待ってください。",
    );
}

#[test]
fn dressing_new_root_extensions_keep_existing_words_raw_and_shoe_authors_selectable() {
    // These three prefixes can gain longer Words; do not assert a byte-equal
    // whole pool, or silently refresh their old vector hashes after expansion.
    for (language, seed, word) in [
        ("zh-Hans", "la lian", "拉链"),
        ("zh-Hans", "niu kou", "纽扣"),
        ("ja", "kutsu", "靴"),
    ] {
        let mut ime = engine(language, seed);
        for (text, kind) in [(word, CandidateKind::Word), (seed, CandidateKind::Literal)] {
            let at = ime
                .candidates()
                .iter()
                .position(|c| c.text == text && c.kind == kind)
                .unwrap();
            if kind == CandidateKind::Word {
                assert!(at < 6);
            }
            ime.select_candidate(at);
            let result = ime.commit(CommitOptions { force: true });
            assert!(result.ok);
            assert_eq!(result.text.as_deref(), Some(text));
            assert!(!ime.commit(CommitOptions { force: true }).ok);
            assert!(ime.undo().unwrap().committed_text.is_empty());
            assert!(ime.undo().is_none());
            ime.seed(seed);
        }
    }
    for sentence in [
        "靴を履いたら、すぐに出ます。",
        "靴は歩きやすいものを選びます。",
    ] {
        old_adoption("ja", "kutsu", "靴", sentence);
    }
}

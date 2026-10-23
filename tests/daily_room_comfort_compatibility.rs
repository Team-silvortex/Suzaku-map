//! Frozen actual dirty-dressing before-room pools; reviewed room roots tested separately.
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
fn room_preserves_eight_actual_english_neighbour_pools() {
    unchanged(
        "en",
        &[
            ("please close the window", 0xa1858c076185c45b),
            ("could you open the window", 0x384554a3fb581b5d),
            ("the window won't", 0x56de1476a912bea5),
            ("the screen is too", 0xf7a391d037d344dc),
            ("could you adjust the brightness", 0x4576bfc9c55da1f9),
            ("my shirt feels tigh", 0xe7a3bc1dd77ea82e),
            ("we've run out of toothpast", 0x39c01d8578dda304),
            ("please check whether ", 0xb284085053f9fd6f),
        ],
    );
    old_adoption(
        "en",
        "my shirt feels tigh",
        "my shirt feels tight",
        "my shirt feels tight when i sit down.",
    );
}

#[test]
fn room_preserves_six_actual_chinese_neighbour_pools() {
    unchanged(
        "zh-Hans",
        &[
            ("chuang hu guan bu shang", 0x3a93fcb5e5f8ef0d),
            ("ping mu tai liang le", 0x60edf2fbb29eed79),
            ("you dian leng", 0x7f016c4960a5040b),
            ("you dian re", 0x73089df966d689a5),
            ("shui qian guan deng", 0x1eafd317bba98bee),
            ("ba la lian la shang", 0x9ed1fe93471a92fb),
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
fn room_preserves_six_actual_japanese_neighbour_pools() {
    unchanged(
        "ja",
        &[
            ("akarusa", 0x9c2795bef6f23f01),
            ("hadazamui", 0x2407c4373ac9f8a5),
            ("mushiatsui", 0x28f8c7cfa84cfef7),
            ("fasuna-", 0xd7219eac701e835e),
            ("kutsu", 0xb846eafefcb16043),
            ("senganryo", 0x0f67ed4192350e0a),
        ],
    );
    old_adoption(
        "ja",
        "fasuna-",
        "ファスナー",
        "ファスナーを最後まで閉めます。",
    );
}

#[test]
fn room_new_root_extensions_retain_old_words_raw_and_ventilation_authors() {
    // No blanket equality claim for roots receiving new longer Words/authors.
    for (language, seed, word) in [
        ("en", "curtain", "curtain"),
        ("zh-Hans", "fang jian", "房间"),
        ("zh-Hans", "chuang lian", "窗帘"),
        ("ja", "kanki", "換気"),
    ] {
        let mut ime = engine(language, seed);
        for text in [word, seed] {
            let at = ime
                .candidates()
                .iter()
                .position(|c| c.text == text)
                .unwrap();
            if text == word {
                assert!(at < 6);
                assert_eq!(ime.candidates()[at].kind, CandidateKind::Word);
            }
            ime.select_candidate(at);
            assert_eq!(ime.snapshot().seed_text, seed);
            let committed = ime.commit(CommitOptions { force: true });
            assert!(committed.ok);
            assert_eq!(committed.text.as_deref(), Some(text));
            assert!(!ime.commit(CommitOptions { force: true }).ok);
            assert!(ime.undo().unwrap().committed_text.is_empty());
            assert!(ime.undo().is_none());
            ime.seed(seed);
        }
    }
    for sentence in [
        "換気のために窓を開けます。",
        "換気をして空気を入れ替えましょう。",
    ] {
        old_adoption("ja", "kanki", "換気", sentence);
    }
}

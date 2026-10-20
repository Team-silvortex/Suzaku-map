//! Actual dirty-storage before-data vectors: full metadata, score bits and order.
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
fn restock_preserves_eight_actual_english_pools_and_prior_storage_adoption() {
    unchanged(
        "en",
        &[
            ("please put these", 0x4462c86a1d9b84ca),
            ("i'll put it back in the", 0x1988533e66ac47fa),
            ("please put it back on the", 0x3b22f6255c3acba0),
            ("let me tidy the", 0xd29a4f12c35c7838),
            ("your video is", 0xe9de704cf76f812a),
            ("i'm taking a different", 0xeff8e5d793a5a005),
            ("i can hear my own", 0xc8dba6e9a444ef3a),
            ("i still have the receipt", 0x7e264bf831bb6733),
        ],
    );
    old_adoption(
        "en",
        "please put these",
        "please put these away",
        "please put these away after you use them.",
    );
}

#[test]
fn restock_preserves_eight_actual_chinese_pools_and_prior_storage_adoption() {
    unchanged(
        "zh-Hans",
        &[
            ("shou na", 0x7b6db572bf58ae7b),
            ("shou na he", 0x83fa9a085c8d95b5),
            ("fang hui yuan chu", 0x9caa0daa67f164cb),
            ("yong wan fang hui yuan chu", 0x95443aa11dd56377),
            ("bang wo shou qi lai", 0x5d84ac6b18206c1a),
            ("wo tui chu chong jin yi xia", 0x50a5a7c697fc0f41),
            ("wo gang shang che", 0xcf339971e1d8c64d),
            ("ni de hua mian ka zhu le", 0xfbcc5afac78e49c8),
        ],
    );
    old_adoption("zh-Hans", "shou na", "收纳", "收纳盒放在柜子里了。");
}

#[test]
fn restock_preserves_eight_actual_japanese_pools_and_prior_storage_adoption() {
    unchanged(
        "ja",
        &[
            // Unsupported spelling remains a literal, not a promised alias for 片付け.
            ("katazuke", 0xce33308c49ce155b),
            ("okibasho", 0xfd88555194ac7edd),
            ("shokki", 0xa613a038c61e7d89),
            ("chawan", 0x88f30b5dcef1ffa9),
            ("hairinaoshimasu", 0xf85e479303f6fc99),
            ("gamengatomatteimasu", 0x854b45ef719d3c21),
            ("machiawasebasho", 0x6947bed44c37ed45),
            ("tsunaginaoshimasu", 0xe3029d13fdc961d1),
        ],
    );
    old_adoption("ja", "kataduke", "片付け", "片付けを手伝ってもらえますか。");
}

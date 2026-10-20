//! Actual dirty-route before-data vectors: full metadata, score bits and order.
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
fn call_preserves_eight_actual_english_pools_and_prior_contact_adoption() {
    unchanged(
        "en",
        &[
            ("can you hear me", 0x85aafa97934fbef6),
            ("your voice keeps", 0x7e9d8a534e367b7f),
            ("the call got", 0x5ebc9de6a6273062),
            ("can we switch to", 0xa1375b3753d0856e),
            ("i'm at the other entrance", 0xa3fcafa885d92e23),
            ("i'm taking a different", 0xeff8e5d793a5a005),
            ("i still have the receipt", 0x7e264bf831bb6733),
            ("please stay where you", 0xd6c82bb16103bea0),
        ],
    );
    old_adoption(
        "en",
        "your voice keeps",
        "your voice keeps breaking",
        "your voice keeps breaking up.",
    );
}

#[test]
fn call_preserves_eight_actual_chinese_pools_and_prior_contact_adoption() {
    unchanged(
        "zh-Hans",
        &[
            ("ni neng ting qing ma", 0x62fa5fc8c5543355),
            ("ni na bian you hui sheng", 0xe284f23a68a9f404),
            ("wo men chong xin bo yi xia", 0x48967debe65b4cc9),
            ("mai ke feng", 0x806680cb79e977b0),
            ("sheng yin you dian xiao", 0xeb7df29123965fb3),
            ("wo zai ling yi ge chu kou", 0x26fdd828be09d8a1),
            ("wo gang shang che", 0xcf339971e1d8c64d),
            ("qian mian feng lu le", 0xd9288cae6d982ecc),
        ],
    );
    old_adoption(
        "zh-Hans",
        "ni neng ting qing ma",
        "你能听清吗",
        "你能听清吗？我把声音调大一点。",
    );
}

#[test]
fn call_preserves_eight_actual_japanese_pools_and_prior_contact_adoption() {
    unchanged(
        "ja",
        &[
            ("tsuuwa", 0x7e4c4f226ca681f7),
            ("denpa", 0xdfb39a804a224b15),
            ("gamenkyouyuu", 0x4b3e69d33fb8dacb),
            ("onsei", 0x5e827bdfe6713b1a),
            ("onryou", 0xdb7046bc85013dd4),
            ("machiawasebasho", 0x6947bed44c37ed45),
            ("betsuru-to", 0xfa40d374942c1b2b),
            ("genzaichi", 0x705d6f07553449db),
        ],
    );
    old_adoption("ja", "tsuuwa", "通話", "通話は今できますか。");
}

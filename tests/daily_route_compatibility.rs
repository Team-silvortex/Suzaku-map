//! Captured from clean 4108bc0 before the route-coordination data append.
//! Complete vectors include text, labels, types, sources, exact score bits and order.
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

fn commit_once(language: &str, seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(language, seed);
    let index = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    assert!(index < PAGE_SIZE, "old first-page choice moved: {text:?}");
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
    assert!(ime.candidates().is_empty());
    assert!(!ime.commit(CommitOptions { force: true }).ok);
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn route_preserves_nine_actual_english_vectors_and_old_travel_choices() {
    unchanged(
        "en",
        &[
            ("i'm on my way", 0x9f159c698822237e),
            ("i'm running a little late", 0xb4e67868b197a533),
            ("i'll be there in a few", 0x0c7efa3559b60109),
            ("i'll call you when", 0xdb946708c06a46cb),
            ("see you at the station", 0xc1d049adc803defe),
            ("could you confirm the address", 0x0d80ed10617424e1),
            ("do i need to book ahea", 0x11062725ca4176fe),
            ("i only just saw your messag", 0x014f05d776df08da),
            ("i still have the receip", 0x25712a41b2257788),
        ],
    );
    for (seed, word, sentence) in [
        (
            "i'm on my way",
            "i'm on my way to",
            "i'm on my way to the station.",
        ),
        (
            "i'll be there in a few",
            "i'll be there in a few minutes",
            "i'll be there in a few minutes.",
        ),
    ] {
        commit_once("en", seed, word, CandidateKind::Word);
        commit_once("en", seed, sentence, CandidateKind::Sentence);
        commit_once("en", seed, seed, CandidateKind::Word);
    }
}

#[test]
fn route_preserves_eight_actual_chinese_vectors_and_existing_meeting_authors() {
    unchanged(
        "zh-Hans",
        &[
            ("kuai dao le", 0xbe38676639d3b91b),
            ("deng wo yi xia", 0xf09cc13615c7c5f3),
            ("zou cuo lu le", 0x29456a8819ccd9df),
            ("zai lu kou peng mian", 0x5a3f05eb2e467d83),
            ("deng wo guo qu", 0xaa5ad9cc8103da2d),
            ("wo yi jing dao men kou le", 0xef0a3dc098216509),
            ("qu hao", 0x9b7c3f776653a495),
            ("gang kan dao ni de xiao xi", 0x18d7d7fbeda36e81),
        ],
    );
    for (seed, word, sentences) in [
        (
            "kuai dao le",
            "快到了",
            ["快到了，请稍等一下。", "快到了，你在门口等我吧。"],
        ),
        (
            "zou cuo lu le",
            "走错路了",
            ["走错路了，我们先停下来问问。", "走错路了，换条路再试试。"],
        ),
    ] {
        commit_once("zh-Hans", seed, word, CandidateKind::Word);
        for sentence in sentences {
            commit_once("zh-Hans", seed, sentence, CandidateKind::Sentence);
        }
        commit_once("zh-Hans", seed, seed, CandidateKind::Literal);
    }
}

#[test]
fn route_preserves_eight_actual_japanese_vectors_and_meeting_time_choices() {
    unchanged(
        "ja",
        &[
            ("shuugoujikan", 0x0de095cc44c9f42f),
            ("yoyakuhenkou", 0xb87d8ca237a8562f),
            ("raitenjikan", 0x97de5fd3bb837e7d),
            ("machijikan", 0xf4edcdf5f5134107),
            ("seiriken", 0xe52cdc95d0e41149),
            ("messe-ji", 0x0f200bfa259678d4),
            ("renraku", 0xd1d7ac502fc53ead),
            ("hoshousho", 0x39ad81a25e6453bd),
        ],
    );
    let seed = "shuugoujikan";
    commit_once("ja", seed, "集合時間", CandidateKind::Word);
    for sentence in [
        "集合時間をもう一度教えてください。",
        "集合時間に間に合いそうです。",
    ] {
        commit_once("ja", seed, sentence, CandidateKind::Sentence);
    }
    commit_once("ja", seed, seed, CandidateKind::Literal);
}

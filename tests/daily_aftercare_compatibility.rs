//! Frozen before the aftercare append, including unreleased messaging/appointments.
//! Related roots may gain useful choices; old authored choices must remain usable.
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
    assert_eq!(ime.snapshot().seed_text, seed);
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
fn aftercare_preserves_nine_actual_english_vectors() {
    unchanged(
        "en",
        &[
            ("receipt", 0xbf24f10a2fc84bd5),
            ("refund", 0xe37fb4f82aa47ac6),
            ("return", 0x728c86fc14a1a36e),
            ("exchange", 0x3e33b99b5e9d3038),
            ("please keep the receipt", 0xc5b1401e68ce34f8),
            ("can i return this if", 0x612024dc5f454fad),
            (
                "i'd like to exchange this for a different",
                0xc6abcac986f76e61,
            ),
            ("do i need to book ahea", 0x11062725ca4176fe),
            ("i only just saw your messag", 0x014f05d776df08da),
        ],
    );
}

#[test]
fn aftercare_preserves_chinese_vectors_and_existing_return_authors() {
    unchanged(
        "zh-Hans",
        &[
            ("xiao piao hai zai", 0x365eff5f14531432),
            ("xu yao liu xiao piao ma", 0xbe75ff8228cfb178),
            ("qu hao", 0x9b7c3f776653a495),
            ("yu yue", 0x9fd222f94f70ddd3),
            ("gang kan dao ni de xiao xi", 0x18d7d7fbeda36e81),
        ],
    );
    for (seed, primary, words, sentences) in [
        (
            "tui huo",
            "退货",
            &["退货", "退货地址", "退货流程", "退货怎么处理"][..],
            ["退货需要保留包装吗？", "退货申请已经提交了。"],
        ),
        (
            "tui kuan",
            "退款",
            &["退款", "退款进度", "退款申请", "退款到账", "退款金额"][..],
            ["退款大概需要多久？", "退款进度可以在哪里查看？"],
        ),
        (
            "huan huo",
            "换货",
            &["换货"][..],
            ["换货需要多久？", "换货时请保留包装。"],
        ),
    ] {
        let ime = engine("zh-Hans", seed);
        assert_eq!(ime.candidates()[0].text, primary);
        assert_eq!(ime.candidates()[0].score.to_bits(), 100_f32.to_bits());
        let retained: Vec<_> = ime
            .candidates()
            .iter()
            .filter(|c| c.kind == CandidateKind::Word && words.contains(&c.text.as_str()))
            .map(|c| c.text.as_str())
            .collect();
        assert_eq!(retained, words);
        commit_once("zh-Hans", seed, primary, CandidateKind::Word, true);
        for sentence in sentences {
            commit_once("zh-Hans", seed, sentence, CandidateKind::Sentence, true);
        }
        commit_once("zh-Hans", seed, seed, CandidateKind::Literal, false);
    }
    for text in ["退货地址请发给我一下。", "退货地址和寄出地址一样吗？"] {
        commit_once("zh-Hans", "tui huo", text, CandidateKind::Sentence, false);
    }
}

#[test]
fn aftercare_preserves_japanese_vectors_receipt_and_return_authors() {
    unchanged(
        "ja",
        &[
            ("seiriken", 0xe52cdc95d0e41149),
            ("yoyakuhenkou", 0xb87d8ca237a8562f),
            ("messe-ji", 0x0f200bfa259678d4),
            ("renraku", 0xd1d7ac502fc53ead),
        ],
    );
    for (seed, word, sentences) in [
        (
            "henpin",
            "返品",
            ["返品できますか。", "返品の期限を教えてください。"],
        ),
        (
            "ryoushuusho",
            "領収書",
            ["領収書をお願いします。", "領収書に名前を入れてください。"],
        ),
    ] {
        let ime = engine("ja", seed);
        assert_eq!(ime.candidates()[0].text, word);
        assert_eq!(ime.candidates()[0].score.to_bits(), 100_f32.to_bits());
        commit_once("ja", seed, word, CandidateKind::Word, true);
        for sentence in sentences {
            commit_once("ja", seed, sentence, CandidateKind::Sentence, true);
        }
        commit_once("ja", seed, seed, CandidateKind::Literal, false);
    }
    commit_once("ja", "koukan", "交換", CandidateKind::Word, true);
    commit_once("ja", "koukan", "koukan", CandidateKind::Literal, false);
    // henkin intentionally changes from a kana-only fallback to the new 返金 Word.
}

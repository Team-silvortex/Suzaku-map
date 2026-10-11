//! Independent Japanese shopping, prices and payment fallback fixtures.
//! These finite presets do not inspect receipts or personal purchases, call
//! a live model, or claim general Japanese morphology or financial advice.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{EntryKind, builtin},
};

const WORDS: &[(&str, &str, &str)] = &[
    ("nedan", "ねだん", "値段"),
    ("kakaku", "かかく", "価格"),
    ("shiharai", "しはらい", "支払い"),
    ("kaikei", "かいけい", "会計"),
    ("genkin", "げんきん", "現金"),
    ("otsuri", "おつり", "お釣り"),
    ("kozeni", "こぜに", "小銭"),
    ("waribiki", "わりびき", "割引"),
    ("zeikomi", "ぜいこみ", "税込"),
    ("zeinuki", "ぜいぬき", "税抜"),
    ("ryoushuusho", "りょうしゅうしょ", "領収書"),
    ("henpin", "へんぴん", "返品"),
    ("koukan", "こうかん", "交換"),
    ("shinagire", "しなぎれ", "品切れ"),
    ("zaiko", "ざいこ", "在庫"),
    ("shichaku", "しちゃく", "試着"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "値段",
        ["値段を教えてください。", "値段を確認してから決めたいです。"],
    ),
    (
        "価格",
        [
            "価格は税込ですか。",
            "価格が予算を超えるので、少し考えます。",
        ],
    ),
    (
        "支払い",
        [
            "支払いはカードでできますか。",
            "支払いの方法を教えてください。",
        ],
    ),
    ("会計", ["会計をお願いします。", "会計は別々にできますか。"]),
    (
        "現金",
        [
            "現金で支払います。",
            "現金が足りないので、カードを使います。",
        ],
    ),
    (
        "割引",
        ["割引はありますか。", "割引の条件を教えてください。"],
    ),
    (
        "領収書",
        ["領収書をお願いします。", "領収書に名前を入れてください。"],
    ),
    ("返品", ["返品できますか。", "返品の期限を教えてください。"]),
];

fn blank_engine() -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime
}
fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = blank_engine();
    ime.seed(seed);
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    ime
}
fn check_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates().iter().any(|candidate| {
            candidate.text == seed && candidate.kind == CandidateKind::Literal
        })
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|candidate| { candidate.source == CandidateSource::Local })
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {text:?} {kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        });
    assert!(
        kind == CandidateKind::Literal || index < PAGE_SIZE,
        "not page one {text:?}, seed={:?}: {:?}",
        ime.snapshot().seed_text,
        ime.candidates()
    );
    index
}
fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let selected = index(ime, text, kind);
    assert_eq!(ime.candidates()[selected].source, CandidateSource::Local);
    ime.select_candidate(selected);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok);
    assert!(duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn shopping_preserves_all_266_previous_readings_and_134_contexts_in_full_order() {
    // Independently captured before shopping: all old kind/separator flags,
    // complete context order and all 267 authored sentences, including time.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 266);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(266) {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for byte in entry
            .reading
            .bytes()
            .chain([0])
            .chain(entry.text.bytes())
            .chain([0])
            .chain(kind.bytes())
            .chain([u8::from(entry.require_separators)])
        {
            readings = (readings ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(readings, 0x301d990b382a696c);
    assert!(ja.continuations().count() >= 134);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut previous_sentence_count = 0;
    for (context, sentences) in ja.continuations().take(134) {
        previous_sentence_count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(previous_sentence_count, 267);
    assert_eq!(contexts, 0x2c2ed9a7d3cfad66);
}

#[test]
fn shopping_additions_match_the_independent_authored_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 16);
    assert_eq!(SENTENCES.len(), 8);
    assert!(ja.readings().len() >= 282);
    assert!(ja.continuations().count() >= 142);
    assert_eq!(
        ja.continuations()
            .take(142)
            .map(|(_, sentences)| sentences.len())
            .sum::<usize>(),
        283
    );
    for (entry, &(_, kana, word)) in ja.readings().iter().skip(266).take(16).zip(WORDS) {
        assert_eq!(entry.reading, kana);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(134).take(8).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn complete_shopping_readings_offer_first_page_words_sentences_and_exact_single_commits() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let ime = engine(&seed);
            check_draft(&ime, &seed);
            commit_once_and_undo(&mut engine(&seed), word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            if let Some(&(_, sentences)) = SENTENCES.iter().find(|(text, _)| *text == word) {
                for sentence in sentences {
                    commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn specific_partial_shopping_readings_keep_words_and_authored_branches_on_page_one() {
    // Specific fixed prefixes, not a promise that all words matching a broad
    // one-kana request displace old choices within the unchanged page budget.
    for (seed, word) in [
        ("ned", "値段"),
        ("kakak", "価格"),
        ("shihara", "支払い"),
        ("kaike", "会計"),
        ("genki", "現金"),
        ("otsur", "お釣り"),
        ("kozen", "小銭"),
        ("waribik", "割引"),
        ("zeikom", "税込"),
        ("zeinuk", "税抜"),
        ("ryoushuush", "領収書"),
        ("henpi", "返品"),
        ("kouka", "交換"),
        ("shinagir", "品切れ"),
        ("zaik", "在庫"),
        ("shichak", "試着"),
        ("ねだ", "値段"),
        ("かか", "価格"),
        ("しはら", "支払い"),
        ("かいけ", "会計"),
        ("げんき", "現金"),
        ("わりび", "割引"),
        ("りょうしゅう", "領収書"),
        ("へんぴ", "返品"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
        if let Some(&(_, sentences)) = SENTENCES.iter().find(|(text, _)| *text == word) {
            for sentence in sentences {
                commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn shopping_horizontal_separators_and_n_apostrophes_preserve_literal_spelling() {
    for (seed, word, sentence) in [
        ("ne dan", "値段", "値段を教えてください。"),
        ("KA\u{3000}KA KU", "価格", "価格は税込ですか。"),
        ("し は らい", "支払い", "支払いはカードでできますか。"),
        ("\tkai kei\t", "会計", "会計をお願いします。"),
        ("  GEN’KIN", "現金", "現金で支払います。"),
        ("wari biki", "割引", "割引はありますか。"),
        ("りょう しゅう しょ", "領収書", "領収書をお願いします。"),
        ("hen'pin", "返品", "返品できますか。"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for (seed, word) in [
        ("gen'kin", "現金"),
        ("HEN’PIN", "返品"),
        ("koukan'", "交換"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn shopping_adoption_space_restore_and_particles_remain_an_editable_draft() {
    for (reading, word, tail, sentence) in [
        ("nedan", "値段", "wo", "値段を教えてください。"),
        ("kakaku", "価格", "HA", "価格は税込ですか。"),
        ("shiharai", "支払い", "の", "支払いの方法を教えてください。"),
        ("kaikei", "会計", "wo", "会計をお願いします。"),
        ("genkin", "現金", "DE", "現金で支払います。"),
        ("waribiki", "割引", "no", "割引の条件を教えてください。"),
        (
            "ryoushuusho",
            "領収書",
            "ni",
            "領収書に名前を入れてください。",
        ),
        ("henpin", "返品", "の", "返品の期限を教えてください。"),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Pure-engine adoption equivalent, not a physical host-key receipt.
        let spaced = format!("{word} ");
        ime.seed(&spaced);
        check_draft(&ime, &spaced);
        index(&ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(&spaced), &spaced, CandidateKind::Literal);
        ime.seed(reading);
        check_draft(&ime, reading);
        index(&ime, word, CandidateKind::Word);
        let continued = format!("{word} {tail}");
        ime.seed(&continued);
        check_draft(&ime, &continued);
        commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(&continued), &continued, CandidateKind::Literal);
    }
}

#[test]
fn shopping_keeps_unknown_text_and_never_joins_control_boundaries() {
    let mut seeds = vec![
        "xyznedan".to_owned(),
        "🙂値段を".into(),
        "https://shiharai".into(),
        "会計woxyz".into(),
        "現金をが".into(),
        "割引の🙂".into(),
    ];
    let boundaries = [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ];
    for boundary in boundaries {
        for (head, tail) in [("ne", "dan"), ("値段", "wo"), ("gen'", "kin")] {
            seeds.push(format!("{head}{boundary}{tail}"));
        }
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        check_draft(&ime, &seed);
        for candidate in ime.candidates() {
            for &(_, sentences) in SENTENCES {
                for sentence in sentences {
                    assert!(
                        !candidate.text.ends_with(sentence),
                        "manufactured {sentence:?} from {seed:?}: {:?}",
                        ime.candidates()
                    );
                }
            }
            for boundary in boundaries {
                if seed.contains(boundary) {
                    assert!(candidate.text.contains(boundary));
                }
            }
        }
        commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-shopping-synthetic-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        self.0.clone().map_or_else(|| Ok(Vec::new()), Err)
    }
}

#[test]
fn shopping_local_candidates_survive_typed_synthetic_failures_and_empty_replies() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("nedan", "値段", "値段を教えてください。"),
            ("shiharai", "支払い", "支払いはカードでできますか。"),
            ("ryoushuusho", "領収書", "領収書をお願いします。"),
        ] {
            let mut ime = blank_engine();
            ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
            ime.seed(seed);
            check_draft(&ime, seed);
            index(&ime, word, CandidateKind::Word);
            index(&ime, sentence, CandidateKind::Sentence);
            let local = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(
                ime.prediction_error(),
                Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
            );
            assert_eq!(ime.candidates(), local);
            check_draft(&ime, seed);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

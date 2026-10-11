//! Independent authored clothing and before-leaving carry preparation fixtures.
//! Finite local fallback does not inspect weather, clothing or personal routines.
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
    ("fuku", "ふく", "服"),
    ("kutsu", "くつ", "靴"),
    ("boushi", "ぼうし", "帽子"),
    ("uwagi", "うわぎ", "上着"),
    ("mochimono", "もちもの", "持ち物"),
    ("kigae", "きがえ", "着替え"),
    ("kutsushita", "くつした", "靴下"),
    ("tebukuro", "てぶくろ", "手袋"),
    ("kaban", "かばん", "鞄"),
    ("suitou", "すいとう", "水筒"),
    ("hankachi", "はんかち", "ハンカチ"),
    ("eri", "えり", "襟"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "服",
        ["服を選んでから出かけます。", "服は動きやすいものにします。"],
    ),
    (
        "靴",
        [
            "靴を履いたら、すぐに出ます。",
            "靴は歩きやすいものを選びます。",
        ],
    ),
    (
        "帽子",
        [
            "帽子を忘れずに持っていきます。",
            "帽子が見つからないので、少し待ってください。",
        ],
    ),
    (
        "上着",
        [
            "上着を持っていくので、少し待ってください。",
            "上着は鞄に入れておきます。",
        ],
    ),
    (
        "持ち物",
        [
            "持ち物を出かける前に確認します。",
            "持ち物は机の上にまとめてあります。",
        ],
    ),
    (
        "着替え",
        [
            "着替えを鞄に入れてから出ます。",
            "着替えが終わったら声をかけます。",
        ],
    ),
];
const HORIZONTAL: &[&str] = &[" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"];
const BOUNDARIES: &[&str] = &[
    "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
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
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == seed && c.kind == CandidateKind::Literal)
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local && (0.0..=100.0).contains(&c.score))
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
}
fn selectable(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {text:?} {kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        })
}
fn first_page(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = selectable(ime, text, kind);
    assert!(
        kind == CandidateKind::Literal || index < PAGE_SIZE,
        "not page one {text:?}: {:?}",
        ime.candidates()
    );
    index
}
fn commit_index_once(ime: &mut XRTabletImeEngine, index: usize, text: &str) {
    assert_eq!(ime.candidates()[index].text, text);
    assert_eq!(ime.candidates()[index].source, CandidateSource::Local);
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok && second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = first_page(ime, text, kind);
    commit_index_once(ime, index, text);
}
fn word_and_sentences(seed: &str, word: &str) {
    let ime = engine(seed);
    draft(&ime, seed);
    first_page(&ime, word, CandidateKind::Word);
    commit_once(&mut engine(seed), word, CandidateKind::Word);
    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    if let Some(&(_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
        for sentence in sentences {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn carry_preserves_all_342_prior_readings_172_contexts_and_343_sentences_in_order() {
    // Frozen before data, including both preceding unreleased daily batches.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 342);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(342) {
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
    assert_eq!(readings, 0xc2b6ada1937e8873);
    assert!(ja.continuations().count() >= 172);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut sentence_count = 0;
    for (context, sentences) in ja.continuations().take(172) {
        sentence_count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentence_count, 343);
    assert_eq!(contexts, 0x73c0414f6ba5d9fa);
}

#[test]
fn carry_matches_the_independent_authored_twelve_reading_six_context_append_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    assert!(ja.readings().len() >= 354 && ja.continuations().count() >= 178);
    assert_eq!(
        ja.continuations()
            .take(178)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        355
    );
    for (entry, &(_, kana, word)) in ja.readings().iter().skip(342).take(12).zip(WORDS) {
        assert_eq!(entry.reading, kana);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(172).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn carry_full_romaji_uppercase_and_kana_offer_first_page_words_and_sentences() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            word_and_sentences(&seed, word);
        }
    }
}

#[test]
fn carry_specific_partials_offer_words_and_both_authored_sentence_branches() {
    for (seed, word) in [
        ("fuk", "服"),
        ("kuts", "靴"),
        ("boush", "帽子"),
        ("ぼう", "帽子"),
        ("uwag", "上着"),
        ("うわ", "上着"),
        ("mochimo", "持ち物"),
        ("もちも", "持ち物"),
        ("kiga", "着替え"),
        ("きが", "着替え"),
        ("kutsushit", "靴下"),
        ("くつし", "靴下"),
        ("tebukur", "手袋"),
        ("てぶく", "手袋"),
        ("kaba", "鞄"),
        ("かば", "鞄"),
        ("suito", "水筒"),
        ("すいと", "水筒"),
        ("hankach", "ハンカチ"),
        ("はんか", "ハンカチ"),
        ("ER", "襟"),
    ] {
        word_and_sentences(seed, word);
    }
}

#[test]
fn carry_horizontal_separators_preserve_literal_spelling_and_local_choices() {
    for &space in HORIZONTAL {
        for (left, right, word) in [
            ("fu", "KU", "服"),
            ("ku", "tsu", "靴"),
            ("bou", "shi", "帽子"),
            ("uwa", "gi", "上着"),
            ("mochi", "mono", "持ち物"),
            ("ki", "gae", "着替え"),
            ("kutsu", "shita", "靴下"),
            ("te", "bukuro", "手袋"),
            ("ka", "ban", "鞄"),
            ("sui", "tou", "水筒"),
            ("han", "kachi", "ハンカチ"),
            ("e", "ri", "襟"),
        ] {
            word_and_sentences(&format!("{space}{left}{space}{right}{space}"), word);
        }
    }
    for (seed, word) in [
        ("もち もの", "持ち物"),
        ("ぼう\u{3000}し", "帽子"),
        ("han'kachi", "ハンカチ"),
        ("HAN’KACHI", "ハンカチ"),
    ] {
        word_and_sentences(seed, word);
    }
}

#[test]
fn carry_adopted_words_particles_and_restoration_stay_editable_until_explicit_commit() {
    for (reading, word, tail, sentence) in [
        ("fuku", "服", "wo", "服を選んでから出かけます。"),
        ("kutsu", "靴", "は", "靴は歩きやすいものを選びます。"),
        ("boushi", "帽子", "WO", "帽子を忘れずに持っていきます。"),
        ("uwagi", "上着", "ha", "上着は鞄に入れておきます。"),
        (
            "mochimono",
            "持ち物",
            "を",
            "持ち物を出かける前に確認します。",
        ),
        ("kigae", "着替え", "ga", "着替えが終わったら声をかけます。"),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(first_page(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Cumulative core reseeding models adoption/restoration only; physical
        // native selection and Backspace require their separate GTK receipt.
        ime.seed(word);
        draft(&ime, word);
        ime.seed(reading);
        draft(&ime, reading);
        first_page(&ime, word, CandidateKind::Word);
        for space in HORIZONTAL {
            let spaced = format!("{word}{space}");
            ime.seed(&spaced);
            draft(&ime, &spaced);
            first_page(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(&spaced), &spaced, CandidateKind::Literal);
            let continued = format!("{spaced}{tail}");
            ime.seed(&continued);
            draft(&ime, &continued);
            commit_once(&mut ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(&continued), &continued, CandidateKind::Literal);
        }
    }
}

#[test]
fn carry_unknown_boundaries_and_existing_long_draft_bound_do_not_invent_a_sentence() {
    let mut seeds = vec![
        "xyzboushi".to_owned(),
        "🙂持ち物を".into(),
        "https://mochimono".into(),
        "boushi@example.invalid".into(),
        "帽子woxyz".into(),
        "服をが".into(),
        "着替えが🙂".into(),
    ];
    for boundary in BOUNDARIES {
        for (head, tail) in [("mochi", "mono"), ("持ち物", "wo")] {
            seeds.push(format!("{head}{boundary}{tail}"));
        }
    }
    // This data-only batch does not extend Japanese's 256-character window.
    for tail in ["boushi", "持ち物wo", "上着ha"] {
        let long = format!("{}{tail}", "前文。".repeat(90));
        assert!(long.chars().count() > 256);
        seeds.push(long);
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
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
            for boundary in BOUNDARIES {
                if seed.contains(boundary) {
                    assert!(candidate.text.contains(boundary));
                }
            }
        }
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
    for (seed, sentence) in [
        ("帽子wo", "帽子を忘れずに持っていきます。"),
        ("持ち物wo", "持ち物を出かける前に確認します。"),
        ("上着ha", "上着は鞄に入れておきます。"),
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        commit_once(&mut ime, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn carry_keeps_old_homophones_page_positions_and_authored_grammar_tails() {
    for seed in ["kanji", "KANJI", "かんじ"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "漢字");
        assert!(
            first_page(&ime, "漢字", CandidateKind::Word)
                < first_page(&ime, "感じ", CandidateKind::Word)
        );
    }
    for seed in ["kou", "KOU", "こう"] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        let candidate = first_page(&ime, "候補", CandidateKind::Word);
        let update = first_page(&ime, "更新", CandidateKind::Word);
        let park = selectable(&ime, "公園", CandidateKind::Word);
        let crossing = selectable(&ime, "交差点", CandidateKind::Word);
        assert!(candidate < update && update < park && park < crossing);
        assert!((PAGE_SIZE..12).contains(&park) && (PAGE_SIZE..12).contains(&crossing));
        commit_index_once(&mut ime, crossing, "交差点");
    }
    for seed in ["bango", "BANGO", "ばんご"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert!(
            first_page(&ime, "晩ご飯", CandidateKind::Word)
                < first_page(&ime, "番号", CandidateKind::Word)
        );
    }
    let mut ime = engine("shit");
    draft(&ime, "shit");
    let down = selectable(&ime, "下", CandidateKind::Word);
    assert!((PAGE_SIZE..12).contains(&down));
    commit_index_once(&mut ime, down, "下");
    for (seed, progress, sentence) in [
        (
            "日本語を勉強shitai",
            "日本語を勉強したい",
            "日本語を勉強したいです。",
        ),
        ("完了shitara", "完了したら", "完了したら連絡してください。"),
        (
            "早起きshita",
            "早起きした",
            "早起きしたので、少し散歩してきます。",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, progress);
        first_page(&ime, sentence, CandidateKind::Sentence);
        assert!(ime.candidates().iter().all(|c| !c.text.contains('下')));
    }
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-carry-synthetic-error"
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
fn carry_local_candidates_survive_five_typed_model_failure_or_empty_result_paths() {
    // Synthetic provider errors, not real HTTP or live-model quality receipts.
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("boushi", "帽子", "帽子を忘れずに持っていきます。"),
            ("mochimono", "持ち物", "持ち物を出かける前に確認します。"),
            ("kigae", "着替え", "着替えが終わったら声をかけます。"),
        ] {
            let mut ime = blank_engine();
            ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
            ime.seed(seed);
            draft(&ime, seed);
            first_page(&ime, word, CandidateKind::Word);
            first_page(&ime, sentence, CandidateKind::Sentence);
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
            draft(&ime, seed);
            commit_once(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

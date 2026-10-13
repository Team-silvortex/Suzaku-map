//! Authored tableware/mealtime fallback; not a general Japanese language benchmark.
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

// Fixed independently before adding the JSON records.
const WORDS: &[(&str, &str, &str)] = &[
    ("okawari", "おかわり", "おかわり"),
    ("torizara", "とりざら", "取り皿"),
    ("chawan", "ちゃわん", "茶碗"),
    ("misoshiru", "みそしる", "味噌汁"),
    ("shokki", "しょっき", "食器"),
    ("moritsuke", "もりつけ", "盛り付け"),
    ("hashi", "はし", "箸"),
    ("yunomi", "ゆのみ", "湯飲み"),
    ("shokugo", "しょくご", "食後"),
    ("toriwake", "とりわけ", "取り分け"),
    ("shirumono", "しるもの", "汁物"),
    ("kozara", "こざら", "小皿"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "おかわり",
        [
            "おかわりをお願いしてもいいですか。",
            "おかわりは少なめでお願いします。",
        ],
    ),
    (
        "取り皿",
        [
            "取り皿をもう一枚お願いします。",
            "取り皿はここに置いておきます。",
        ],
    ),
    ("茶碗", ["茶碗を並べておきます。", "茶碗は棚に戻しました。"]),
    (
        "味噌汁",
        ["味噌汁を先にいただきます。", "味噌汁はもうできています。"],
    ),
    (
        "食器",
        ["食器を一緒に片付けましょう。", "食器は私が洗います。"],
    ),
    (
        "盛り付け",
        [
            "盛り付けを手伝ってもらえますか。",
            "盛り付けが終わったら運びます。",
        ],
    ),
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
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
            .all(|c| c.source == CandidateSource::Local)
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
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let i = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()));
    assert!(
        kind == CandidateKind::Literal || i < PAGE_SIZE,
        "{text:?}: {:?}",
        ime.candidates()
    );
    i
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    assert!(!ime.commit(CommitOptions { force: true }).ok);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn choices(seed: &str, word: &str) {
    let ime = engine(seed);
    draft(&ime, seed);
    // An unchanged kana word shares the Literal candidate; conversion must not duplicate it.
    let kind = if seed == word {
        CandidateKind::Literal
    } else {
        CandidateKind::Word
    };
    commit_once(&mut engine(seed), word, kind);
    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
        for sentence in sentences {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn mealtime_preserves_all_366_readings_184_contexts_and_367_sentences() {
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 366 && ja.continuations().count() >= 184);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for e in ja.readings().iter().take(366) {
        let kind = match e.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for b in e
            .reading
            .bytes()
            .chain([0])
            .chain(e.text.bytes())
            .chain([0])
            .chain(kind.bytes())
            .chain([u8::from(e.require_separators)])
        {
            reading_hash = (reading_hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(reading_hash, 0x67d789c292e6c648);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut count = 0;
    for (context, ss) in ja.continuations().take(184) {
        count += ss.len();
        for s in std::iter::once(context).chain(ss.iter().map(String::as_str)) {
            for b in s.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(count, 367);
    assert_eq!(context_hash, 0xfc65282c3e495fd3);
}

#[test]
fn mealtime_matches_fixed_twelve_readings_and_six_context_append_window() {
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 378 && ja.continuations().count() >= 190);
    for (entry, &(_, kana, word)) in ja.readings()[366..378].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, ss), &(expected, sentences)) in
        ja.continuations().skip(184).take(6).zip(SENTENCES)
    {
        assert_eq!(context, expected);
        assert_eq!(ss.iter().map(String::as_str).collect::<Vec<_>>(), sentences);
    }
}

#[test]
fn mealtime_full_and_specific_partial_readings_offer_words_and_both_sentences() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
            romaji[..romaji.len() - 1].into(),
        ] {
            choices(&seed, word);
        }
    }
}

#[test]
fn mealtime_spaces_adoption_and_particles_stay_editable_until_explicit_commit() {
    for (reading, word, sentence) in [
        ("torizara", "取り皿", "取り皿をもう一枚お願いします。"),
        ("misoshiru", "味噌汁", "味噌汁を先にいただきます。"),
        ("shokki", "食器", "食器を一緒に片付けましょう。"),
    ] {
        for space in [" ", "\t", "\u{a0}", "\u{3000}"] {
            choices(&format!("{space}{reading}{space}"), word);
            let mut ime = engine(reading);
            ime.select_candidate(index(&ime, word, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(word));
            assert!(ime.snapshot().committed_text.is_empty());
            // Engine-level reseeding, not a physical keyboard/adoption receipt.
            for tail in ["", "wo", "を"] {
                let continued = format!("{word}{space}{tail}");
                ime.seed(&continued);
                draft(&ime, &continued);
                commit_once(&mut ime, sentence, CandidateKind::Sentence);
                commit_once(&mut engine(&continued), &continued, CandidateKind::Literal);
            }
            ime.seed(reading);
            draft(&ime, reading);
            index(&ime, word, CandidateKind::Word);
        }
    }
}

#[test]
fn mealtime_invalid_boundaries_and_long_drafts_keep_literal_and_old_ranks() {
    let mut seeds = vec![
        "https://shokki".into(),
        "torizara@example.invalid".into(),
        "🙂味噌汁wo".into(),
        "食器woxyz".into(),
    ];
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("tori{boundary}zara"));
        seeds.push(format!("取り皿{boundary}wo"));
    }
    seeds.push(format!("{}食器wo", "前文。".repeat(90)));
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(ime.candidates().iter().all(|c| {
            !SENTENCES
                .iter()
                .flat_map(|(_, ss)| ss)
                .any(|s| c.text.ends_with(s))
        }));
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
    for (seed, initial, first, second) in [
        ("kanji", "漢字", "漢字", "感じ"),
        ("bango", "ばんご", "晩ご飯", "番号"),
    ] {
        let ime = engine(seed);
        assert_eq!(ime.candidates()[0].text, initial);
        assert!(index(&ime, first, CandidateKind::Word) < index(&ime, second, CandidateKind::Word));
    }
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-mealtime-synthetic-error"
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
fn mealtime_candidates_survive_missing_failed_timeout_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        let mut ime = engine("");
        ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
        ime.seed("torizara");
        draft(&ime, "torizara");
        index(&ime, "取り皿", CandidateKind::Word);
        index(&ime, SENTENCES[1].1[0], CandidateKind::Sentence);
        let before = ime.candidates().to_vec();
        let deadline = Instant::now() + Duration::from_secs(2);
        while ime.prediction_pending() && Instant::now() < deadline {
            ime.poll_prediction();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
        assert_eq!(
            ime.prediction_error(),
            Some(&error.unwrap_or(LlmProviderError::NoCandidates))
        );
        assert_eq!(ime.candidates(), before);
        commit_once(&mut ime, SENTENCES[1].1[0], CandidateKind::Sentence);
    }
}

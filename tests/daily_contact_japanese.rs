//! Independent Japanese phone, signal and message-contact fallback fixtures.
//! These finite authored expressions do not read contacts or messages, inspect
//! a device or account, make calls, send messages, or require a live model.
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
    ("tsuuwa", "つうわ", "通話"),
    ("chakushin", "ちゃくしん", "着信"),
    ("rusubandenwa", "るすばんでんわ", "留守番電話"),
    ("denpa", "でんぱ", "電波"),
    ("tsuushin", "つうしん", "通信"),
    ("kaisen", "かいせん", "回線"),
    ("setsuzoku", "せつぞく", "接続"),
    ("kengai", "けんがい", "圏外"),
    ("bangou", "ばんごう", "番号"),
    ("keitai", "けいたい", "携帯"),
    ("tsuuchi", "つうち", "通知"),
    ("kidoku", "きどく", "既読"),
    ("midoku", "みどく", "未読"),
    ("onryou", "おんりょう", "音量"),
    ("juuden", "じゅうでん", "充電"),
    ("saisou", "さいそう", "再送"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "通話",
        [
            "通話は今できますか。",
            "通話が途切れたので、もう一度かけます。",
        ],
    ),
    (
        "着信",
        [
            "着信に気づいたら折り返してください。",
            "着信の音が聞こえませんでした。",
        ],
    ),
    (
        "留守番電話",
        [
            "留守番電話に伝言を残しました。",
            "留守番電話を確認してから連絡します。",
        ],
    ),
    (
        "電波",
        [
            "電波が弱いので、少し待ってください。",
            "電波が届く場所からかけ直します。",
        ],
    ),
    (
        "接続",
        [
            "接続を確認してから送ります。",
            "接続が切れたので、もう一度試します。",
        ],
    ),
    (
        "番号",
        [
            "番号をもう一度教えてください。",
            "番号が違っていないか確認します。",
        ],
    ),
    (
        "充電",
        [
            "充電が終わったら連絡します。",
            "充電が必要なので、後でかけ直します。",
        ],
    ),
    (
        "通知",
        [
            "通知が来たら教えてください。",
            "通知を確認してから返信します。",
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
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
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
    commit_once(&mut engine(seed), word, CandidateKind::Word);
    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    if let Some(&(_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
        for sentence in sentences {
            let mut selected = engine(seed);
            commit_once(&mut selected, sentence, CandidateKind::Sentence);
            selected.seed(seed);
            draft(&selected, seed);
            commit_once(&mut selected, seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn contact_preserves_all_298_previous_readings_and_150_contexts_in_full_order() {
    // Independently captured before contact: all old flags and all 299
    // authored sentences, including the shopping and transit expansions.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 298);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(298) {
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
    assert_eq!(readings, 0x0dc0525ce2b5f11e);
    assert!(ja.continuations().count() >= 150);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut previous_sentence_count = 0;
    for (context, sentences) in ja.continuations().take(150) {
        previous_sentence_count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(previous_sentence_count, 299);
    assert_eq!(contexts, 0xdce824869c74e0bf);
}

#[test]
fn contact_additions_match_the_independent_authored_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 16);
    assert_eq!(SENTENCES.len(), 8);
    assert!(ja.readings().len() >= 314 && ja.continuations().count() >= 158);
    assert_eq!(
        ja.continuations()
            .take(158)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        315
    );
    for (entry, &(_, kana, word)) in ja.readings().iter().skip(298).take(16).zip(WORDS) {
        assert_eq!(entry.reading, kana);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(150).take(8).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn contact_full_romaji_case_and_kana_offer_first_page_words_and_both_sentence_branches() {
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
fn contact_precise_partials_and_number_ambiguity_preserve_old_choices_and_exact_commits() {
    // Avoid broad つう/でん requests: their old words and authored contexts
    // retain finite slots. These fixed specific prefixes are not a promise
    // that every abbreviated reading can place every sentence on page one.
    for (seed, word) in [
        ("tsuuw", "通話"),
        ("chakushi", "着信"),
        ("rusubandenw", "留守番電話"),
        ("denp", "電波"),
        ("tsuushi", "通信"),
        ("kaise", "回線"),
        ("setsuzok", "接続"),
        ("kenga", "圏外"),
        ("keita", "携帯"),
        ("tsuuch", "通知"),
        ("kidok", "既読"),
        ("midok", "未読"),
        ("onryo", "音量"),
        ("juude", "充電"),
        ("saiso", "再送"),
        ("ちゃくし", "着信"),
        ("るすばんでん", "留守番電話"),
        ("せつぞ", "接続"),
        ("じゅうで", "充電"),
    ] {
        word_and_sentences(seed, word);
    }
    // ばんご still completes the prior 晩ご飯 before new 番号. The old
    // word has no authored context; both number sentences fit on page one
    // without removing that word or either raw kana form.
    for seed in ["bango", "BANGO", "ばんご"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "ばんご");
        assert_eq!(
            ime.candidates()[0].kind,
            if seed == "ばんご" {
                CandidateKind::Literal
            } else {
                CandidateKind::Unspecified
            }
        );
        assert!(
            first_page(&ime, "晩ご飯", CandidateKind::Word)
                < first_page(&ime, "番号", CandidateKind::Word)
        );
        word_and_sentences(seed, "番号");
    }
}

#[test]
fn contact_six_horizontal_separators_and_n_apostrophes_preserve_exact_raw_spelling() {
    for &space in HORIZONTAL {
        for (left, right, word) in [
            ("tsuu", "WA", "通話"),
            ("chaku", "shin", "着信"),
            ("rusuban", "denwa", "留守番電話"),
            ("den", "pa", "電波"),
            ("setsu", "zoku", "接続"),
            ("ban", "gou", "番号"),
            ("juu", "den", "充電"),
            ("tsuu", "chi", "通知"),
        ] {
            let seed = format!("{space}{left}{space}{right}{space}");
            word_and_sentences(&seed, word);
        }
    }
    for (seed, word) in [
        ("rusuban'denwa", "留守番電話"),
        ("RUSUBAN’DENWA", "留守番電話"),
        ("den'pa", "電波"),
        ("BAN’GOU", "番号"),
        ("ON’RYOU", "音量"),
        ("tsuushin'", "通信"),
    ] {
        word_and_sentences(seed, word);
    }
}

#[test]
fn contact_adoption_particles_space_and_raw_restoration_stay_editable() {
    for (reading, word, tail, sentence) in [
        ("tsuuwa", "通話", "HA", "通話は今できますか。"),
        (
            "chakushin",
            "着信",
            "ni",
            "着信に気づいたら折り返してください。",
        ),
        (
            "rusubandenwa",
            "留守番電話",
            "を",
            "留守番電話を確認してから連絡します。",
        ),
        (
            "denpa",
            "電波",
            "ga",
            "電波が弱いので、少し待ってください。",
        ),
        ("setsuzoku", "接続", "wo", "接続を確認してから送ります。"),
        ("bangou", "番号", "wo", "番号をもう一度教えてください。"),
        ("juuden", "充電", "が", "充電が終わったら連絡します。"),
        ("tsuuchi", "通知", "wo", "通知を確認してから返信します。"),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(first_page(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Pure-engine host-update equivalents, not physical-key receipts.
        let spaced = format!("{word} ");
        ime.seed(&spaced);
        draft(&ime, &spaced);
        first_page(&ime, sentence, CandidateKind::Sentence);
        commit_once(&mut engine(&spaced), &spaced, CandidateKind::Literal);
        ime.seed(reading);
        draft(&ime, reading);
        first_page(&ime, word, CandidateKind::Word);
        let continued = format!("{word} {tail}");
        ime.seed(&continued);
        draft(&ime, &continued);
        commit_once(&mut ime, sentence, CandidateKind::Sentence);
        ime.seed(&continued);
        draft(&ime, &continued);
        commit_once(&mut ime, &continued, CandidateKind::Literal);
    }
}

#[test]
fn contact_protected_unknown_and_seven_line_boundaries_do_not_manufacture_authored_sentences() {
    let mut seeds = vec![
        "xyztsuuwa".to_owned(),
        "🙂通話は".into(),
        "https://setsuzoku".into(),
        "tsuuwa@example.invalid".into(),
        "番号nixyz".into(),
        "通知をが".into(),
        "充電が🙂".into(),
    ];
    for boundary in BOUNDARIES {
        for (head, tail) in [("den'", "pa"), ("着信", "ni"), ("rusuban'", "denwa")] {
            seeds.push(format!("{head}{boundary}{tail}"));
        }
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
    // Positive controls distinguish intended written-word progress from the
    // conflicting tails and protected strings above; no arbitrary prefix scan.
    for (seed, sentence) in [
        ("通話HA", "通話は今できますか。"),
        ("通知wo", "通知を確認してから返信します。"),
        ("充電ga", "充電が終わったら連絡します。"),
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        commit_once(&mut ime, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn contact_keeps_old_homophones_kou_pages_and_standalone_shit_selection() {
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
        for sentence in ["公園で少し休みませんか。", "公園まで歩いてみましょう。"]
        {
            first_page(&ime, sentence, CandidateKind::Sentence);
        }
        commit_index_once(&mut ime, crossing, "交差点");
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
    let mut ime = engine("shit");
    draft(&ime, "shit");
    let down = selectable(&ime, "下", CandidateKind::Word);
    assert!((PAGE_SIZE..12).contains(&down));
    commit_index_once(&mut ime, down, "下");
    commit_once(&mut engine("shit"), "shit", CandidateKind::Literal);
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-contact-synthetic-error"
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
fn contact_local_candidates_survive_missing_failed_timed_out_http_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("tsuuwa", "通話", "通話は今できますか。"),
            ("setsuzoku", "接続", "接続を確認してから送ります。"),
            (
                "rusubandenwa",
                "留守番電話",
                "留守番電話に伝言を残しました。",
            ),
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

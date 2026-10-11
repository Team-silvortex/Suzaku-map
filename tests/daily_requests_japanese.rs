//! Independent authored Japanese object-finding and small-request fixtures.
//! This finite offline fallback does not find real objects, read personal
//! activity, execute requests, or require a live model.
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
    ("kashite", "かして", "貸して"),
    ("sagashimono", "さがしもの", "探し物"),
    ("wasuremono", "わすれもの", "忘れ物"),
    ("otoshimono", "おとしもの", "落とし物"),
    ("kagi", "かぎ", "鍵"),
    ("bunbougu", "ぶんぼうぐ", "文房具"),
    ("saifu", "さいふ", "財布"),
    ("kasa", "かさ", "傘"),
    ("megane", "めがね", "眼鏡"),
    ("enpitsu", "えんぴつ", "鉛筆"),
    ("keshigomu", "けしごむ", "消しゴム"),
    ("hasami", "はさみ", "ハサミ"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "貸して",
        [
            "貸してもらってもいいですか。",
            "貸してくれてありがとうございます。",
        ],
    ),
    (
        "探し物",
        [
            "探し物があるので、少し待ってください。",
            "探し物を一緒に探してもらえますか。",
        ],
    ),
    (
        "忘れ物",
        ["忘れ物がないか確認しましょう。", "忘れ物を取りに戻ります。"],
    ),
    (
        "落とし物",
        [
            "落とし物を見つけたので、受付に届けます。",
            "落とし物の問い合わせはどこですか。",
        ],
    ),
    (
        "鍵",
        [
            "鍵を少し貸してもらえますか。",
            "鍵が見つからないので、一緒に探してください。",
        ],
    ),
    (
        "文房具",
        [
            "文房具を少し借りてもいいですか。",
            "文房具は使った後で戻します。",
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
            .all(|c| { c.source == CandidateSource::Local && (0.0..=100.0).contains(&c.score) })
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
fn requests_preserve_all_330_prior_readings_and_166_contexts_in_full_order() {
    // Fixed before this batch, including the preceding unreleased routine data.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 330);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(330) {
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
    assert_eq!(readings, 0x6502d54dd5cd0326);
    assert!(ja.continuations().count() >= 166);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut sentence_count = 0;
    for (context, sentences) in ja.continuations().take(166) {
        sentence_count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentence_count, 331);
    assert_eq!(contexts, 0x016bb344859ef531);
}

#[test]
fn requests_match_the_independent_authored_append_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    assert!(ja.readings().len() >= 342 && ja.continuations().count() >= 172);
    assert_eq!(
        ja.continuations()
            .take(172)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        343
    );
    for (entry, &(_, kana, word)) in ja.readings().iter().skip(330).take(12).zip(WORDS) {
        assert_eq!(entry.reading, kana);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(166).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn requests_complete_romaji_case_and_kana_offer_first_page_words_and_sentences() {
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
fn requests_specific_partials_offer_first_page_words_and_both_sentence_branches() {
    for (seed, word) in [
        ("kashit", "貸して"),
        ("かし", "貸して"),
        ("sagashimon", "探し物"),
        ("さがしも", "探し物"),
        ("wasuremon", "忘れ物"),
        ("わすれも", "忘れ物"),
        ("otoshimon", "落とし物"),
        ("おとしも", "落とし物"),
        ("kag", "鍵"),
        ("KAG", "鍵"),
        ("bunboug", "文房具"),
        ("ぶんぼう", "文房具"),
        ("saif", "財布"),
        ("keshigom", "消しゴム"),
        ("megan", "眼鏡"),
        ("enpits", "鉛筆"),
        ("hasam", "ハサミ"),
    ] {
        word_and_sentences(seed, word);
    }
}

#[test]
fn requests_horizontal_separators_preserve_exact_spelling_and_choices() {
    for &space in HORIZONTAL {
        for (left, right, word) in [
            ("ka", "SHITE", "貸して"),
            ("sagashi", "mono", "探し物"),
            ("wasure", "mono", "忘れ物"),
            ("otoshi", "mono", "落とし物"),
            ("ka", "gi", "鍵"),
            ("bun", "bougu", "文房具"),
            ("sai", "fu", "財布"),
            ("ka", "sa", "傘"),
            ("me", "gane", "眼鏡"),
            ("en", "pitsu", "鉛筆"),
            ("keshi", "gomu", "消しゴム"),
            ("ha", "sami", "ハサミ"),
        ] {
            word_and_sentences(&format!("{space}{left}{space}{right}{space}"), word);
        }
    }
    for (seed, word) in [
        ("さがし もの", "探し物"),
        ("ぶん\u{3000}ぼうぐ", "文房具"),
        ("BUN’BOUGU", "文房具"),
        ("bun'bougu", "文房具"),
    ] {
        word_and_sentences(seed, word);
    }
}

#[test]
fn requests_adoption_restoration_and_particles_remain_editable_before_explicit_commit() {
    for (reading, word, tail, sentence) in [
        ("kashite", "貸して", "mo", "貸してもらってもいいですか。"),
        (
            "sagashimono",
            "探し物",
            "GA",
            "探し物があるので、少し待ってください。",
        ),
        ("wasuremono", "忘れ物", "を", "忘れ物を取りに戻ります。"),
        (
            "otoshimono",
            "落とし物",
            "no",
            "落とし物の問い合わせはどこですか。",
        ),
        ("kagi", "鍵", "wo", "鍵を少し貸してもらえますか。"),
        ("bunbougu", "文房具", "は", "文房具は使った後で戻します。"),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(first_page(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Cumulative core drafts model host adoption/restoration. They are
        // not native key receipts or physical Backspace acceptance.
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
fn requests_protected_unknown_line_separated_and_long_text_never_invents_a_request() {
    let mut seeds = vec![
        "xyzkashite".to_owned(),
        "🙂探し物が".into(),
        "https://wasuremono".into(),
        "kagi@example.invalid".into(),
        "鍵woxyz".into(),
        "文房具をが".into(),
        "貸しても🙂".into(),
    ];
    for boundary in BOUNDARIES {
        for (head, tail) in [("sagashi", "mono"), ("探し物", "ga")] {
            seeds.push(format!("{head}{boundary}{tail}"));
        }
    }
    // Existing Japanese long-draft bound remains explicit. A long unrelated
    // prefix may retain literal/conversion choices, not a new finite sentence.
    for tail in ["kashite", "鍵wo", "文房具ha"] {
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
        ("探し物ga", "探し物があるので、少し待ってください。"),
        ("鍵wo", "鍵を少し貸してもらえますか。"),
        ("文房具ha", "文房具は使った後で戻します。"),
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        commit_once(&mut ime, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn requests_keep_prior_homophones_bounded_pages_and_authored_tails() {
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
        "ja-requests-synthetic-error"
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
fn requests_local_candidates_survive_typed_model_failures_and_empty_results() {
    // Typed synthetic results, not real HTTP or live-model quality acceptance.
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("kashite", "貸して", "貸してもらってもいいですか。"),
            ("wasuremono", "忘れ物", "忘れ物を取りに戻ります。"),
            ("kagi", "鍵", "鍵を少し貸してもらえますか。"),
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

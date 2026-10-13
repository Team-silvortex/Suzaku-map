//! Authored everyday device vocabulary, not device inspection or repair advice.
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

// Fixed before appending data; no model, user device state or installed pack is needed.
const WORDS: &[(&str, &str, &str)] = &[
    ("juudenki", "じゅうでんき", "充電器"),
    ("denchi", "でんち", "電池"),
    ("zanryou", "ざんりょう", "残量"),
    ("iyahon", "いやほん", "イヤホン"),
    ("dengen", "でんげん", "電源"),
    ("akarusa", "あかるさ", "明るさ"),
    ("sumaho", "すまほ", "スマホ"),
    ("pasokon", "ぱそこん", "パソコン"),
    ("rimokon", "りもこん", "リモコン"),
    ("konsento", "こんせんと", "コンセント"),
    ("hontai", "ほんたい", "本体"),
    ("onsei", "おんせい", "音声"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "充電器",
        [
            "充電器を少し借りてもいいですか。",
            "充電器は鞄に入っています。",
        ],
    ),
    (
        "電池",
        ["電池が切れそうです。", "電池はまだ残っていますか。"],
    ),
    (
        "残量",
        [
            "残量を確認してから出かけます。",
            "残量が少ないので、後で連絡します。",
        ],
    ),
    (
        "イヤホン",
        [
            "イヤホンを忘れたので、音を小さくします。",
            "イヤホンはどこに置きましたか。",
        ],
    ),
    (
        "電源",
        [
            "電源が入らないので、少し待ってください。",
            "電源を切ってから鞄に入れます。",
        ],
    ),
    (
        "明るさ",
        [
            "明るさを少し下げてもいいですか。",
            "明るさはこのくらいで大丈夫です。",
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
    index(ime, seed, CandidateKind::Literal);
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
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()))
}
fn first_page(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert!(
        index(ime, text, kind) < PAGE_SIZE,
        "{text:?}: {:?}",
        ime.candidates()
    );
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
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok && second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn devices_preserve_all_402_prior_readings_202_contexts_and_403_sentences() {
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 402 && ja.continuations().count() >= 202);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(402) {
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
    assert_eq!(readings, 0xd28c00ac80cec38b);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut count = 0;
    for (context, sentences) in ja.continuations().take(202) {
        count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(count, 403);
    assert_eq!(contexts, 0x74d3370d7b3e89de);
}

#[test]
fn devices_match_the_fixed_twelve_readings_and_six_context_append_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    assert!(ja.readings().len() >= 414 && ja.continuations().count() >= 208);
    for (entry, &(_, kana, word)) in ja.readings()[402..414].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(202).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(208)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        415
    );
}

#[test]
fn devices_full_romaji_case_and_kana_offer_words_sentences_and_raw_literal() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            first_page(&ime, word, CandidateKind::Word);
            commit_once(&mut engine(&seed), word, CandidateKind::Word);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
            if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
                for sentence in sentences {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                    commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn devices_specific_partials_and_horizontal_separators_remain_selectable() {
    // Explicit near-complete readings, not a promise for arbitrary short prefixes
    // or page-one placement when old shorter words also match (e.g. 充電).
    for (seed, word) in [
        ("juudenk", "充電器"),
        ("dench", "電池"),
        ("zanryo", "残量"),
        ("iyaho", "イヤホン"),
        ("denge", "電源"),
        ("akarus", "明るさ"),
        ("ざんりょ", "残量"),
        ("いやほ", "イヤホン"),
        ("でんげ", "電源"),
        ("あかる", "明るさ"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        for sentence in SENTENCES
            .iter()
            .find(|(context, _)| *context == word)
            .unwrap()
            .1
        {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
    for separator in [" ", "\t", "\u{a0}", "\u{3000}"] {
        let seed = format!("{separator}juu{separator}DENKI{separator}");
        let ime = engine(&seed);
        draft(&ime, &seed);
        first_page(&ime, "充電器", CandidateKind::Word);
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}

#[test]
fn devices_converted_contexts_and_adoption_restoration_do_not_commit_early() {
    for (reading, word, tail, sentence) in [
        ("juudenki", "充電器", "wo", SENTENCES[0].1[0]),
        ("denchi", "電池", "が", SENTENCES[1].1[0]),
        ("zanryou", "残量", "WO", SENTENCES[2].1[0]),
        ("iyahon", "イヤホン", "は", SENTENCES[3].1[1]),
        ("dengen", "電源", "ga", SENTENCES[4].1[0]),
        ("akarusa", "明るさ", "を", SENTENCES[5].1[0]),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        // Core cumulative reseeding models host adoption/restoration, not native keys.
        for seed in [
            word.to_owned(),
            format!("{word} {tail}"),
            format!("{word}\u{3000}{tail}"),
        ] {
            ime.seed(&seed);
            draft(&ime, &seed);
            first_page(&ime, sentence, CandidateKind::Sentence);
            ime.seed(reading);
            draft(&ime, reading);
            index(&ime, word, CandidateKind::Word);
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn devices_url_unknown_and_line_boundaries_preserve_literal_without_sentence_leaks() {
    for seed in [
        "https://juudenki",
        "denchi@example.invalid",
        "xyzzanryou",
        "充電器woxyz",
        "🙂電源ga",
        "juu\ndenki",
        "残量\r\nwo",
        "イヤホン\u{2028}ha",
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        assert!(ime.candidates().iter().all(|c| {
            !SENTENCES
                .iter()
                .flat_map(|(_, ss)| ss)
                .any(|sentence| c.text.ends_with(sentence))
        }));
        commit_once(&mut ime, seed, CandidateKind::Literal);
    }
}

#[test]
fn devices_keep_prior_homophone_order_charging_context_and_authored_grammar() {
    for (seed, initial, first, second) in [
        ("kanji", "漢字", "漢字", "感じ"),
        ("bango", "ばんご", "晩ご飯", "番号"),
    ] {
        let ime = engine(seed);
        assert_eq!(ime.candidates()[0].text, initial);
        assert!(index(&ime, first, CandidateKind::Word) < index(&ime, second, CandidateKind::Word));
    }
    let charge = engine("juuden");
    assert_eq!(charge.candidates()[0].text, "充電");
    for sentence in [
        "充電が終わったら連絡します。",
        "充電が必要なので、後でかけ直します。",
    ] {
        first_page(&charge, sentence, CandidateKind::Sentence);
    }
    let grammar = engine("日本語を勉強shitai");
    assert_eq!(grammar.candidates()[0].text, "日本語を勉強したい");
    first_page(
        &grammar,
        "日本語を勉強したいです。",
        CandidateKind::Sentence,
    );
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-devices-synthetic-error"
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
fn devices_fallback_survives_typed_failure_and_empty_model_results() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        let mut ime = engine("");
        ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
        ime.seed("juudenki");
        draft(&ime, "juudenki");
        first_page(&ime, "充電器", CandidateKind::Word);
        first_page(&ime, SENTENCES[0].1[0], CandidateKind::Sentence);
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
        commit_once(&mut ime, SENTENCES[0].1[0], CandidateKind::Sentence);
    }
}

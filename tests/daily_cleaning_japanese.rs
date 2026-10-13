//! Authored home-cleaning vocabulary, not regional waste-disposal rules or real input.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::{
        japanese::romaji_to_hiragana,
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
    lexicon::{EntryKind, builtin},
};

// Fixed against the 462-reading baseline before appending this batch. Natural
// Kana spelling remains Kana; it is not replaced by uncommon Kanji for tests.
const WORDS: &[(&str, &str, &str)] = &[
    ("soujiki", "そうじき", "掃除機"),
    ("zoukin", "ぞうきん", "雑巾"),
    ("mizubuki", "みずぶき", "水拭き"),
    ("karabuki", "からぶき", "乾拭き"),
    ("fukisouji", "ふきそうじ", "拭き掃除"),
    ("yogore", "よごれ", "汚れ"),
    ("gomidashi", "ごみだし", "ごみ出し"),
    ("bunbetsu", "ぶんべつ", "分別"),
    ("gomibukuro", "ごみぶくろ", "ごみ袋"),
    ("shigengomi", "しげんごみ", "資源ごみ"),
    ("kanki", "かんき", "換気"),
    ("yuka", "ゆか", "床"),
    ("shimi", "しみ", "染み"),
    ("hokori", "ほこり", "ほこり"),
    ("houki", "ほうき", "ほうき"),
    ("chiritori", "ちりとり", "ちり取り"),
    ("baketsu", "ばけつ", "バケツ"),
    ("burashi", "ぶらし", "ブラシ"),
    ("suponji", "すぽんじ", "スポンジ"),
    ("moerugomi", "もえるごみ", "燃えるごみ"),
    ("moenaigomi", "もえないごみ", "燃えないごみ"),
    ("kamikuzu", "かみくず", "紙くず"),
    ("akibin", "あきびん", "空き瓶"),
    ("akikan", "あきかん", "空き缶"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "掃除機",
        [
            "掃除機をかけてから出かけます。",
            "掃除機のごみを捨てました。",
        ],
    ),
    (
        "雑巾",
        ["雑巾を絞って床を拭きます。", "雑巾は洗って干しておきます。"],
    ),
    (
        "水拭き",
        ["水拭きで汚れを落とします。", "水拭きの後はよく乾かします。"],
    ),
    (
        "乾拭き",
        ["乾拭きで仕上げます。", "乾拭き用の布を用意しました。"],
    ),
    (
        "拭き掃除",
        [
            "拭き掃除は週末にまとめてします。",
            "拭き掃除をしたら部屋が明るくなりました。",
        ],
    ),
    (
        "汚れ",
        [
            "汚れが気になるので拭いておきます。",
            "汚れがきれいに落ちました。",
        ],
    ),
    (
        "ごみ出し",
        [
            "ごみ出しは朝のうちに済ませます。",
            "ごみ出しの日を確認しましょう。",
        ],
    ),
    (
        "分別",
        ["分別の方法を確認します。", "分別してから袋に入れましょう。"],
    ),
    (
        "ごみ袋",
        [
            "ごみ袋を新しいものに替えます。",
            "ごみ袋が少なくなりました。",
        ],
    ),
    (
        "資源ごみ",
        [
            "資源ごみの出し方を確認しました。",
            "資源ごみの回収日はいつですか。",
        ],
    ),
    (
        "換気",
        [
            "換気のために窓を開けます。",
            "換気をして空気を入れ替えましょう。",
        ],
    ),
    (
        "床",
        [
            "床に置いたものを片付けます。",
            "床が乾くまで待ってください。",
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
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()))
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
fn cleaning_romaji_inputs_preserve_existing_kana_spelling_rules() {
    for &(romaji, kana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
    }
    for (seed, kana) in [
        ("zoukinn", "ぞうきん"),
        ("akibinn", "あきびん"),
        ("akikann", "あきかん"),
        ("bunbetsu", "ぶんべつ"),
        ("kanki", "かんき"),
        ("soujiki", "そうじき"),
    ] {
        assert_eq!(romaji_to_hiragana(seed), kana, "{seed}");
    }
    // ou spells おう; it is not the loanword long-vowel mark ー.
    assert_eq!(romaji_to_hiragana("ho-ki"), "ほーき");
}

#[test]
fn cleaning_preserve_all_462_prior_readings_232_contexts_and_463_sentences() {
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 462 && ja.continuations().count() >= 232);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(462) {
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
    assert_eq!(readings, 0x56cf269c5ad4764f);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut count = 0;
    for (context, sentences) in ja.continuations().take(232) {
        count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(count, 463);
    assert_eq!(contexts, 0xb743db46271f2b69);
}

#[test]
fn cleaning_match_fixed_24_readings_and_12_context_append_windows() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SENTENCES.len(), 12);
    assert!(ja.readings().len() >= 486 && ja.continuations().count() >= 244);
    for (entry, &(_, kana, word)) in ja.readings()[462..486].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(232).take(12).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(244)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        487
    );
}

#[test]
fn cleaning_full_romaji_case_and_kana_offer_natural_words_sentences_and_literal() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            // Natural Kana input equal to its output is the untouched Literal,
            // while the same word reached from Roman letters is a Word.
            let kind = if seed == word {
                CandidateKind::Literal
            } else {
                CandidateKind::Word
            };
            first_page(&ime, word, kind);
            commit_once(&mut engine(&seed), word, kind);
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
fn cleaning_specific_partials_keep_selectable_words_and_both_sentences() {
    // Fixed near-complete inputs, not a promise for every short prefix.
    for (seed, word) in [
        ("soujik", "掃除機"),
        ("zouki", "雑巾"),
        ("mizubuk", "水拭き"),
        ("karabuk", "乾拭き"),
        ("fukisouj", "拭き掃除"),
        ("yogor", "汚れ"),
        ("gomidash", "ごみ出し"),
        ("bunbets", "分別"),
        ("gomibukur", "ごみ袋"),
        ("shigengom", "資源ごみ"),
        ("kank", "換気"),
        ("yuk", "床"),
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
            index(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
    for (seed, word) in [
        ("zoukinn", "雑巾"),
        ("akibinn", "空き瓶"),
        ("akikann", "空き缶"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
    }
}

#[test]
fn cleaning_horizontal_separators_preserve_whole_readings_and_raw_text() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (left, right, word) in [
            ("sou", "JIKI", "掃除機"),
            ("mizu", "BUKI", "水拭き"),
            ("gomi", "BUKURO", "ごみ袋"),
            ("ho", "kori", "ほこり"),
        ] {
            let seed = format!("{separator}{left}{separator}{right}{separator}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            first_page(&ime, word, CandidateKind::Word);
            if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
                for sentence in sentences {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                }
            }
            commit_once(&mut engine(&seed), word, CandidateKind::Word);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn cleaning_adopted_context_particles_and_restoration_never_commit_early() {
    for (word, tail, sentence) in [
        ("掃除機", "wo", SENTENCES[0].1[0]),
        ("雑巾", "ha", SENTENCES[1].1[1]),
        ("水拭き", "de", SENTENCES[2].1[0]),
        ("乾拭き", "で", SENTENCES[3].1[0]),
        ("拭き掃除", "HA", SENTENCES[4].1[0]),
        ("汚れ", "ga", SENTENCES[5].1[1]),
        ("ごみ出し", "no", SENTENCES[6].1[1]),
        ("分別", "の", SENTENCES[7].1[0]),
        ("ごみ袋", "WO", SENTENCES[8].1[0]),
        ("資源ごみ", "no", SENTENCES[9].1[0]),
        ("換気", "wo", SENTENCES[10].1[1]),
        ("床", "ni", SENTENCES[11].1[0]),
    ] {
        let reading = WORDS.iter().find(|(_, _, text)| *text == word).unwrap().0;
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        // Core adoption/restoration through cumulative seeds, not native keys or focus.
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
fn cleaning_url_unknown_and_line_boundaries_do_not_leak_authored_sentences() {
    for seed in [
        "https://soujiki.example.invalid",
        "gomidashi@example.invalid",
        "xyzzoukin",
        "掃除機dexyz",
        "🙂ごみ出しha",
        "mizu\nbuki",
        "雑巾\r\nwo",
        "分別\u{2028}no",
        "換気\u{2029}wo",
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
fn cleaning_keep_prior_homophones_longer_words_and_grammar() {
    let homophones = engine("kanji");
    assert_eq!(homophones.candidates()[0].text, "漢字");
    assert!(
        index(&homophones, "漢字", CandidateKind::Word)
            < index(&homophones, "感じ", CandidateKind::Word)
    );
    for (seed, word) in [
        ("kankisen", "換気扇"),
        ("mizumore", "水漏れ"),
        ("buntan", "分担"),
        ("bunnbougu", "文房具"),
        ("houkou", "方向"),
        ("kataduke", "片付け"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, word);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
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
        "ja-cleaning-synthetic-error"
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
fn cleaning_all_twelve_contexts_survive_typed_and_empty_model_failures() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        Some(LlmProviderError::InvalidResponse),
        None,
    ] {
        for &(word, sentences) in SENTENCES {
            let reading = WORDS.iter().find(|(_, _, text)| *text == word).unwrap().0;
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                first_page(&ime, sentence, CandidateKind::Sentence);
            }
            let before = ime.candidates().to_vec();
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
            assert_eq!(ime.candidates(), before);
            commit_once(&mut ime, sentences[0], CandidateKind::Sentence);
        }
    }
}

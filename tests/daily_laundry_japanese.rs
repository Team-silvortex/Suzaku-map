//! Authored laundry, drying, bringing clothes in and everyday clothing fallback.
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

// Fixed before changing data; arrays and expected append windows are independent.
const WORDS: &[(&str, &str, &str)] = &[
    ("sentakumono", "せんたくもの", "洗濯物"),
    ("sentakuki", "せんたくき", "洗濯機"),
    ("senzai", "せんざい", "洗剤"),
    ("heyaboshi", "へやぼし", "部屋干し"),
    ("juunanzai", "じゅうなんざい", "柔軟剤"),
    ("fudangi", "ふだんぎ", "普段着"),
    ("irui", "いるい", "衣類"),
    ("monohoshi", "ものほし", "物干し"),
    ("sentakubasami", "せんたくばさみ", "洗濯ばさみ"),
    ("hadagi", "はだぎ", "肌着"),
    ("sode", "そで", "袖"),
    ("suso", "すそ", "裾"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "洗濯物",
        [
            "洗濯物を取り込んでおきます。",
            "洗濯物を干すのを手伝ってもらえますか。",
        ],
    ),
    (
        "洗濯機",
        [
            "洗濯機を回しておきます。",
            "洗濯機が終わったら教えてください。",
        ],
    ),
    (
        "洗剤",
        ["洗剤はどこにありますか。", "洗剤が少なくなってきました。"],
    ),
    (
        "部屋干し",
        [
            "部屋干しにしておきます。",
            "部屋干しの服はまだ乾いていません。",
        ],
    ),
    (
        "柔軟剤",
        ["柔軟剤は使いますか。", "柔軟剤はいつもの棚にあります。"],
    ),
    (
        "普段着",
        [
            "普段着に着替えてから出かけます。",
            "普段着はこの引き出しに入っています。",
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
    let selected = index(ime, text, kind);
    commit_at(ime, text, selected);
}
fn commit_at(ime: &mut XRTabletImeEngine, text: &str, selected: usize) {
    ime.select_candidate(selected);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn choices(seed: &str, word: &str) {
    let ime = engine(seed);
    draft(&ime, seed);
    // Exact Kana words, when present, share the Literal rather than duplicating it.
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
fn laundry_preserve_all_390_readings_196_contexts_and_391_sentences() {
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 390 && ja.continuations().count() >= 196);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for e in ja.readings().iter().take(390) {
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
    assert_eq!(reading_hash, 0x839b8d848820222d);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut count = 0;
    for (context, ss) in ja.continuations().take(196) {
        count += ss.len();
        for s in std::iter::once(context).chain(ss.iter().map(String::as_str)) {
            for b in s.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(count, 391);
    assert_eq!(context_hash, 0xd27f1b9ea8bb7ccc);
}

#[test]
fn laundry_match_fixed_twelve_readings_and_six_context_append_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    assert!(ja.readings().len() >= 402 && ja.continuations().count() >= 202);
    for (entry, &(_, kana, word)) in ja.readings()[390..402].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, ss), &(expected, sentences)) in
        ja.continuations().skip(196).take(6).zip(SENTENCES)
    {
        assert_eq!(context, expected);
        assert_eq!(ss.iter().map(String::as_str).collect::<Vec<_>>(), sentences);
    }
}

#[test]
fn laundry_full_romaji_kana_case_and_specific_partials_offer_words_and_sentences() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            choices(&seed, word);
        }
    }
    // These ten partials keep both sentence branches on the first page.
    // The two ambiguous sentaku compounds have separate paged checks below.
    for (seed, word) in [
        ("senza", "洗剤"),
        ("heyabosh", "部屋干し"),
        ("juunanza", "柔軟剤"),
        ("fudang", "普段着"),
        ("iru", "衣類"),
        ("monohosh", "物干し"),
        ("sentakubasam", "洗濯ばさみ"),
        ("hadag", "肌着"),
        ("sod", "袖"),
        ("sus", "裾"),
    ] {
        choices(seed, word);
    }
}

#[test]
fn laundry_ambiguous_sentaku_partials_keep_both_sentences_across_bounded_pages() {
    for (seed, word, sentences) in [
        ("sentakumon", "洗濯物", SENTENCES[0].1),
        ("sentakuk", "洗濯機", SENTENCES[1].1),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
        for (branch, sentence) in sentences.into_iter().enumerate() {
            let selected = ime
                .candidates()
                .iter()
                .position(|c| c.text == sentence && c.kind == CandidateKind::Sentence)
                .unwrap_or_else(|| panic!("{seed}: missing {sentence:?}: {:?}", ime.candidates()));
            assert!(selected < 2 * PAGE_SIZE);
            if branch == 0 {
                assert!(selected < PAGE_SIZE);
            }
            // These readings also match the old 選択 conversion; the second
            // authored sentence remains selectable without overriding it.
            if seed == "sentakumon" && branch == 1 {
                assert!(selected >= PAGE_SIZE);
            }
            commit_at(&mut engine(seed), sentence, selected);
        }
    }
}

#[test]
fn laundry_horizontal_separators_preserve_literal_and_local_choices() {
    for space in [" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"] {
        for (left, right, word) in [
            ("sentaku", "MONO", "洗濯物"),
            ("sentaku", "ki", "洗濯機"),
            ("sen", "zai", "洗剤"),
            ("heya", "boshi", "部屋干し"),
            ("juunan", "zai", "柔軟剤"),
            ("fudan", "gi", "普段着"),
        ] {
            choices(&format!("{space}{left}{space}{right}{space}"), word);
        }
    }
    for seed in ["sen'zai", "SEN’ZAI", "せん ざい"] {
        choices(seed, "洗剤");
    }
}

#[test]
fn laundry_adopted_words_and_particle_progress_stay_editable_until_commit() {
    for (reading, word, tail, sentence) in [
        (
            "sentakumono",
            "洗濯物",
            "wo",
            "洗濯物を取り込んでおきます。",
        ),
        (
            "sentakuki",
            "洗濯機",
            "が",
            "洗濯機が終わったら教えてください。",
        ),
        ("senzai", "洗剤", "HA", "洗剤はどこにありますか。"),
        ("heyaboshi", "部屋干し", "ni", "部屋干しにしておきます。"),
        ("juunanzai", "柔軟剤", "は", "柔軟剤は使いますか。"),
        (
            "fudangi",
            "普段着",
            "ni",
            "普段着に着替えてから出かけます。",
        ),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Core reseeding models adoption/restoration, not physical native keys.
        for space in [" ", "\t", "\u{a0}", "\u{3000}"] {
            for suffix in ["", tail] {
                let continued = format!("{word}{space}{suffix}");
                ime.seed(&continued);
                draft(&ime, &continued);
                commit_once(&mut ime, sentence, CandidateKind::Sentence);
                commit_once(&mut engine(&continued), &continued, CandidateKind::Literal);
            }
        }
        ime.seed(reading);
        draft(&ime, reading);
        index(&ime, word, CandidateKind::Word);
    }
}

#[test]
fn laundry_unknown_line_and_long_draft_boundaries_preserve_the_complete_raw_text() {
    const LINES: &[&str] = &[
        "\n", "\r", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ];
    let mut seeds = vec![
        "https://sentakumono".into(),
        "senzai@example.invalid".into(),
        "xyzheyaboshi".into(),
        "🙂洗濯物wo".into(),
        "洗剤haxyz".into(),
        "部屋干しをが".into(),
        "普段着に🙂".into(),
    ];
    for boundary in LINES {
        seeds.push(format!("sentaku{boundary}mono"));
        seeds.push(format!("洗濯物{boundary}wo"));
    }
    for tail in ["sentakumono", "洗濯機wo", "普段着ni"] {
        let seed = format!("{}{tail}", "前文。".repeat(90));
        assert!(seed.chars().count() > 256);
        seeds.push(seed);
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        for candidate in ime.candidates() {
            assert!(
                !SENTENCES
                    .iter()
                    .flat_map(|(_, ss)| ss)
                    .any(|s| candidate.text.ends_with(s))
            );
            for boundary in LINES {
                if seed.contains(boundary) {
                    assert!(candidate.text.contains(boundary));
                }
            }
        }
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

#[test]
fn laundry_keeps_old_homophones_selection_sentences_and_authored_grammar_tails() {
    for (seed, initial, first, second) in [
        ("kanji", "漢字", "漢字", "感じ"),
        ("bango", "ばんご", "晩ご飯", "番号"),
    ] {
        let ime = engine(seed);
        assert_eq!(ime.candidates()[0].text, initial);
        assert!(index(&ime, first, CandidateKind::Word) < index(&ime, second, CandidateKind::Word));
    }
    for seed in ["sentaku", "SENTAKU", "せんたく"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "選択");
        for sentence in [
            "選択に迷ったら相談してください。",
            "選択する前に比べてみましょう。",
        ] {
            index(&ime, sentence, CandidateKind::Sentence);
        }
    }
    choices("おかわり", "おかわり");
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
        index(&ime, sentence, CandidateKind::Sentence);
        assert!(ime.candidates().iter().all(|c| !c.text.contains('下')));
    }
}

struct Unavailable(Option<LlmProviderError>);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-laundry-synthetic-error"
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
fn laundry_local_candidates_survive_missing_failed_timeout_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        let mut ime = engine("");
        ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
        ime.seed("sentakumono");
        draft(&ime, "sentakumono");
        index(&ime, "洗濯物", CandidateKind::Word);
        index(&ime, SENTENCES[0].1[0], CandidateKind::Sentence);
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

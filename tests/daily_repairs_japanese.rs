//! Authored household problem reports and appointment requests, not repair advice.
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

// Independent expectations recorded before appending any resource data.
const WORDS: &[(&str, &str, &str)] = &[
    ("mizumore", "みずもれ", "水漏れ"),
    ("amamori", "あまもり", "雨漏り"),
    ("jaguchi", "じゃぐち", "蛇口"),
    ("koshou", "こしょう", "故障"),
    ("shuuri", "しゅうり", "修理"),
    ("tenken", "てんけん", "点検"),
    ("denkyuu", "でんきゅう", "電球"),
    ("amido", "あみど", "網戸"),
    ("madowaku", "まどわく", "窓枠"),
    ("suidou", "すいどう", "水道"),
    ("haisuikou", "はいすいこう", "排水口"),
    ("kankisen", "かんきせん", "換気扇"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "水漏れ",
        [
            "水漏れを見てもらえますか。",
            "水漏れの場所を写真で送ります。",
        ],
    ),
    (
        "雨漏り",
        [
            "雨漏りがあるので、一度見てもらえますか。",
            "雨漏りは昨日から続いています。",
        ],
    ),
    (
        "蛇口",
        [
            "蛇口から水が漏れています。",
            "蛇口の調子を見てもらえますか。",
        ],
    ),
    (
        "故障",
        [
            "故障したので、修理をお願いしたいです。",
            "故障の状況を説明します。",
        ],
    ),
    (
        "修理",
        [
            "修理に来てもらえる日時を教えてください。",
            "修理の予定を確認したいです。",
        ],
    ),
    (
        "点検",
        ["点検に来てもらえますか。", "点検の日程を相談したいです。"],
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
fn repairs_preserve_all_378_readings_190_contexts_and_379_sentences() {
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 378 && ja.continuations().count() >= 190);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for e in ja.readings().iter().take(378) {
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
    assert_eq!(reading_hash, 0x70788a8f178b38d9);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut count = 0;
    for (context, ss) in ja.continuations().take(190) {
        count += ss.len();
        for s in std::iter::once(context).chain(ss.iter().map(String::as_str)) {
            for b in s.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(count, 379);
    assert_eq!(context_hash, 0x346dffcc34c6991a);
}

#[test]
fn repairs_match_fixed_twelve_readings_and_six_context_append_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    assert!(ja.readings().len() >= 390 && ja.continuations().count() >= 196);
    for (entry, &(_, kana, word)) in ja.readings()[378..390].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, ss), &(expected, sentences)) in
        ja.continuations().skip(190).take(6).zip(SENTENCES)
    {
        assert_eq!(context, expected);
        assert_eq!(ss.iter().map(String::as_str).collect::<Vec<_>>(), sentences);
    }
}

#[test]
fn repairs_full_romaji_kana_case_and_specific_partials_offer_both_sentence_branches() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            choices(&seed, word);
        }
    }
    // Seventeen explicit partials plus one repeated full reading (しゅうり);
    // these do not promise arbitrary short/broad prefixes.
    for (seed, word) in [
        ("mizumor", "水漏れ"),
        ("みずも", "水漏れ"),
        ("amamor", "雨漏り"),
        ("あまも", "雨漏り"),
        ("jaguch", "蛇口"),
        ("じゃぐ", "蛇口"),
        ("kosho", "故障"),
        ("こしょ", "故障"),
        ("shuur", "修理"),
        ("しゅうり", "修理"),
        ("tenke", "点検"),
        ("てんけ", "点検"),
        ("denkyu", "電球"),
        ("amid", "網戸"),
        ("madowak", "窓枠"),
        ("suido", "水道"),
        ("haisuiko", "排水口"),
        ("kankise", "換気扇"),
    ] {
        choices(seed, word);
    }
}

#[test]
fn repairs_horizontal_separators_preserve_literal_and_local_choices() {
    for space in [" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"] {
        for (left, right, word) in [
            ("mizu", "MORE", "水漏れ"),
            ("ama", "mori", "雨漏り"),
            ("ja", "guchi", "蛇口"),
            ("ko", "shou", "故障"),
            ("shuu", "ri", "修理"),
            ("ten", "ken", "点検"),
        ] {
            choices(&format!("{space}{left}{space}{right}{space}"), word);
        }
    }
    for seed in ["ten'ken", "TEN’KEN", "てん けん"] {
        choices(seed, "点検");
    }
}

#[test]
fn repairs_adopted_words_and_particle_progress_remain_editable_until_commit() {
    for (reading, word, tail, sentence) in [
        ("mizumore", "水漏れ", "wo", "水漏れを見てもらえますか。"),
        ("amamori", "雨漏り", "は", "雨漏りは昨日から続いています。"),
        ("jaguchi", "蛇口", "NO", "蛇口の調子を見てもらえますか。"),
        ("koshou", "故障", "no", "故障の状況を説明します。"),
        (
            "shuuri",
            "修理",
            "ni",
            "修理に来てもらえる日時を教えてください。",
        ),
        ("tenken", "点検", "に", "点検に来てもらえますか。"),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Core cumulative reseeding models adoption; not a physical keyboard receipt.
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
fn repairs_unknown_line_and_long_draft_boundaries_do_not_manufacture_sentences() {
    let mut seeds = vec![
        "https://mizumore".into(),
        "jaguchi@example.invalid".into(),
        "xyzmizumore".into(),
        "🙂修理ni".into(),
        "水漏れwoxyz".into(),
        "雨漏りをが".into(),
        "点検が🙂".into(),
    ];
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("mizu{boundary}more"));
        seeds.push(format!("水漏れ{boundary}wo"));
    }
    for tail in ["mizumore", "修理ni", "点検no"] {
        let seed = format!("{}{tail}", "前文。".repeat(90));
        assert!(seed.chars().count() > 256);
        seeds.push(seed);
    }
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
}

#[test]
fn repairs_keep_old_homophones_mealtime_and_authored_grammar_tails() {
    for (seed, initial, first, second) in [
        ("kanji", "漢字", "漢字", "感じ"),
        ("bango", "ばんご", "晩ご飯", "番号"),
    ] {
        let ime = engine(seed);
        assert_eq!(ime.candidates()[0].text, initial);
        assert!(index(&ime, first, CandidateKind::Word) < index(&ime, second, CandidateKind::Word));
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
        "ja-repairs-synthetic-error"
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
fn repairs_local_candidates_survive_missing_failed_timeout_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        let mut ime = engine("");
        ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
        ime.seed("mizumore");
        draft(&ime, "mizumore");
        index(&ime, "水漏れ", CandidateKind::Word);
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

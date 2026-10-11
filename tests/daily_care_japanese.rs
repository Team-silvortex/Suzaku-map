//! Independent authored washing, tidying and before-leaving fallback fixtures.
//! Finite local examples do not provide medical advice or inspect personal routines.
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
    ("hamigaki", "はみがき", "歯磨き"),
    ("sengan", "せんがん", "洗顔"),
    ("tearai", "てあらい", "手洗い"),
    ("kataduke", "かたづけ", "片付け"),
    ("tojimari", "とじまり", "戸締まり"),
    ("shoutou", "しょうとう", "消灯"),
    ("sekken", "せっけん", "石鹸"),
    ("haburashi", "はぶらし", "歯ブラシ"),
    ("hamigakiko", "はみがきこ", "歯磨き粉"),
    ("yokushitsu", "よくしつ", "浴室"),
    ("senmenjo", "せんめんじょ", "洗面所"),
    ("taoru", "たおる", "タオル"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "歯磨き",
        [
            "歯磨きを済ませてから出かけます。",
            "歯磨きはもう終わりました。",
        ],
    ),
    (
        "洗顔",
        [
            "洗顔を済ませてから着替えます。",
            "洗顔が終わったら声をかけます。",
        ],
    ),
    (
        "手洗い",
        [
            "手洗いを済ませたら、席に戻ります。",
            "手洗いは先に済ませました。",
        ],
    ),
    (
        "片付け",
        [
            "片付けを手伝ってもらえますか。",
            "片付けが終わったら出かけます。",
        ],
    ),
    (
        "戸締まり",
        [
            "戸締まりを確認してから出ます。",
            "戸締まりは私が確認します。",
        ],
    ),
    (
        "消灯",
        ["消灯してから部屋を出ます。", "消灯は私がしておきます。"],
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
fn care_preserves_all_354_prior_readings_178_contexts_and_355_sentences_in_order() {
    // Frozen before data, including the preceding unreleased routine, request and carry data.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 354);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(354) {
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
    assert_eq!(readings, 0x0715e85641eae48d);
    assert!(ja.continuations().count() >= 178);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut sentence_count = 0;
    for (context, sentences) in ja.continuations().take(178) {
        sentence_count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentence_count, 355);
    assert_eq!(contexts, 0x120d08d140b6f4e9);
}

#[test]
fn care_matches_the_independent_authored_twelve_reading_six_context_append_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 12);
    assert_eq!(SENTENCES.len(), 6);
    assert!(ja.readings().len() >= 366 && ja.continuations().count() >= 184);
    assert_eq!(
        ja.continuations()
            .take(184)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        367
    );
    for (entry, &(_, kana, word)) in ja.readings().iter().skip(354).take(12).zip(WORDS) {
        assert_eq!(entry.reading, kana);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(178).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn care_full_romaji_uppercase_and_kana_offer_first_page_words_and_sentences() {
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
fn care_specific_partials_offer_words_and_both_authored_sentence_branches() {
    // Specific tails, not broad "sen"/"sho": older words retain their priority.
    for (seed, word) in [
        ("hamigak", "歯磨き"),
        ("はみが", "歯磨き"),
        ("senga", "洗顔"),
        ("せんが", "洗顔"),
        ("teara", "手洗い"),
        ("てあら", "手洗い"),
        ("kataduk", "片付け"),
        ("かたづ", "片付け"),
        ("tojimar", "戸締まり"),
        ("とじま", "戸締まり"),
        ("shouto", "消灯"),
        ("しょうと", "消灯"),
        ("sekke", "石鹸"),
        ("せっけ", "石鹸"),
        ("haburash", "歯ブラシ"),
        ("はぶら", "歯ブラシ"),
        ("hamigakik", "歯磨き粉"),
        ("はみがきこ", "歯磨き粉"),
        ("yokushits", "浴室"),
        ("よくし", "浴室"),
        ("senmenj", "洗面所"),
        ("せんめん", "洗面所"),
        ("TAOR", "タオル"),
        ("たお", "タオル"),
    ] {
        word_and_sentences(seed, word);
    }
}

#[test]
fn care_horizontal_separators_preserve_literal_spelling_and_local_choices() {
    for &space in HORIZONTAL {
        for (left, right, word) in [
            ("ha", "MIGAKI", "歯磨き"),
            ("sen", "gan", "洗顔"),
            ("te", "arai", "手洗い"),
            ("kata", "duke", "片付け"),
            ("toji", "mari", "戸締まり"),
            ("shou", "tou", "消灯"),
            ("se", "kken", "石鹸"),
            ("ha", "burashi", "歯ブラシ"),
            ("hamigaki", "ko", "歯磨き粉"),
            ("yoku", "shitsu", "浴室"),
            ("sen", "menjo", "洗面所"),
            ("ta", "oru", "タオル"),
        ] {
            word_and_sentences(&format!("{space}{left}{space}{right}{space}"), word);
        }
    }
    for (seed, word) in [
        ("はみ がき", "歯磨き"),
        ("とじ\u{3000}まり", "戸締まり"),
        ("sen'gan", "洗顔"),
        ("SEN’GAN", "洗顔"),
    ] {
        word_and_sentences(seed, word);
    }
}

#[test]
fn care_adopted_words_particles_and_restoration_stay_editable_until_explicit_commit() {
    for (reading, word, tail, sentence) in [
        (
            "hamigaki",
            "歯磨き",
            "wo",
            "歯磨きを済ませてから出かけます。",
        ),
        ("sengan", "洗顔", "が", "洗顔が終わったら声をかけます。"),
        (
            "tearai",
            "手洗い",
            "WO",
            "手洗いを済ませたら、席に戻ります。",
        ),
        ("kataduke", "片付け", "ga", "片付けが終わったら出かけます。"),
        (
            "tojimari",
            "戸締まり",
            "を",
            "戸締まりを確認してから出ます。",
        ),
        ("shoutou", "消灯", "ha", "消灯は私がしておきます。"),
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
fn care_unknown_boundaries_and_existing_long_draft_bound_do_not_invent_a_sentence() {
    let mut seeds = vec![
        "xyztearai".to_owned(),
        "🙂戸締まりを".into(),
        "https://tojimari".into(),
        "tearai@example.invalid".into(),
        "手洗いwoxyz".into(),
        "歯磨きをが".into(),
        "消灯が🙂".into(),
    ];
    for boundary in BOUNDARIES {
        for (head, tail) in [("toji", "mari"), ("戸締まり", "wo")] {
            seeds.push(format!("{head}{boundary}{tail}"));
        }
    }
    // This data-only batch does not extend Japanese's 256-character window.
    for tail in ["tearai", "戸締まりwo", "片付けha"] {
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
        ("手洗いwo", "手洗いを済ませたら、席に戻ります。"),
        ("戸締まりwo", "戸締まりを確認してから出ます。"),
        ("片付けga", "片付けが終わったら出かけます。"),
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        commit_once(&mut ime, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn care_keeps_old_homophones_page_positions_and_authored_grammar_tails() {
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
        "ja-care-synthetic-error"
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
fn care_local_candidates_survive_five_typed_model_failure_or_empty_result_paths() {
    // Synthetic provider errors, not real HTTP or live-model quality receipts.
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("tearai", "手洗い", "手洗いを済ませたら、席に戻ります。"),
            ("tojimari", "戸締まり", "戸締まりを確認してから出ます。"),
            ("shoutou", "消灯", "消灯は私がしておきます。"),
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

#[test]
fn care_keeps_canonical_du_kana_separate_from_unsupported_zu_spelling() {
    // Canonical かたづけ uses the existing du -> づ row. A finite data
    // addition does not introduce a katazuke alias or general ず/づ correction.
    for seed in ["kataduke", "KATADUKE", "かたづけ"] {
        word_and_sentences(seed, "片付け");
    }
    for seed in ["katazuke", "KATAZUKE", "かたずけ"] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        assert!(ime.candidates().iter().all(|candidate| {
            candidate.text != "片付け"
                && !SENTENCES
                    .iter()
                    .any(|(_, sentences)| sentences.contains(&candidate.text.as_str()))
        }));
        commit_once(&mut ime, seed, CandidateKind::Literal);
    }
}

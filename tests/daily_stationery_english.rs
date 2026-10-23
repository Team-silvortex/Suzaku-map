//! Finite paper-and-pencil fallback; authored contexts do not imply arbitrary short-prefix coverage.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    languages::{
        english::is_known_english_word,
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
    lexicon::{WordLayer, builtin},
};

const NEW_FORMS: [&str; 8] = [
    "pen", "pens", "pencil", "pencils", "eraser", "ruler", "sticky", "ink",
];
const CONTEXTS: [&str; 6] = [
    "inside my pencil case i keep",
    "drawing a straight line is easier with a",
    "the ink in this pen has",
    "the sticky note next to my keyboard is",
    "the eraser next to my notebook is",
    "once i've finished writing i'll put the",
];
const WORDS: [[&str; 2]; 6] = [
    ["pens", "pencils"],
    ["ruler", "pencil"],
    ["run", "left"],
    ["for", "from"],
    ["small", "clean"],
    ["pen", "ruler"],
];
const PARTIALS: [[&str; 2]; 6] = [
    ["pen", "pencil"],
    ["rule", "penci"],
    ["ru", "lef"],
    ["fo", "fro"],
    ["smal", "clea"],
    ["pe", "rule"],
];
const SENTENCES: [[&str; 2]; 6] = [
    [
        "inside my pencil case i keep pens for taking notes.",
        "inside my pencil case i keep pencils for drawing small maps.",
    ],
    [
        "drawing a straight line is easier with a ruler on the paper.",
        "drawing a straight line is easier with a pencil than with this pen.",
    ],
    [
        "the ink in this pen has run out, so i'll use a pencil.",
        "the ink in this pen has left a mark on my notebook.",
    ],
    [
        "the sticky note next to my keyboard is for the shopping list.",
        "the sticky note next to my keyboard is from our meeting this morning.",
    ],
    [
        "the eraser next to my notebook is small enough to fit in my pencil case.",
        "the eraser next to my notebook is clean, so it won't leave a mark on the paper.",
    ],
    [
        "once i've finished writing i'll put the pen back in the drawer.",
        "once i've finished writing i'll put the ruler next to my notebook.",
    ],
];

type Case = (&'static str, &'static str, &'static str, &'static str);
fn cases() -> impl Iterator<Item = Case> {
    CONTEXTS.iter().enumerate().flat_map(|(i, &context)| {
        (0..2).map(move |j| (context, WORDS[i][j], PARTIALS[i][j], SENTENCES[i][j]))
    })
}
fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig::default());
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.candidates()[0].text, seed);
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    assert!(ime.candidates().iter().all(|c| {
        c.source == CandidateSource::Local
            && c.text.starts_with(seed)
            && c.score.is_finite()
            && (0.0..=100.0).contains(&c.score)
    }));
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
        .take(PAGE_SIZE)
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}
fn choices(seed: &str, word: &str, sentence: &str) {
    let ime = engine(seed);
    draft(&ime, seed);
    index(&ime, word, CandidateKind::Word);
    index(&ime, sentence, CandidateKind::Sentence);
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let seed = ime.snapshot().seed_text;
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn raw_once(seed: &str) {
    let mut ime = engine(seed);
    draft(&ime, seed);
    let kind = ime.candidates()[0].kind;
    assert!(matches!(kind, CandidateKind::Literal | CandidateKind::Word));
    commit_once(&mut ime, seed, kind);
}
fn indexed_occurrences(layers: &[WordLayer]) -> Vec<(String, usize)> {
    let mut seen = HashSet::new();
    layers
        .iter()
        .flat_map(|layer| {
            layer
                .words
                .iter()
                .map(String::as_str)
                .chain(
                    layer
                        .next_words
                        .iter()
                        .flat_map(|(_, ws)| ws.iter().map(String::as_str)),
                )
                .chain(layer.sentences.iter().flat_map(|sentence| {
                    sentence
                        .split(|ch: char| !(ch.is_ascii_alphabetic() || matches!(ch, '\'' | '’')))
                        .filter(|word| !word.is_empty())
                }))
        })
        .enumerate()
        .filter_map(|(rank, word)| {
            let word = word.replace('’', "'").to_ascii_lowercase();
            seen.insert(word.clone()).then_some((word, rank))
        })
        .collect()
}
fn fnv(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes.into_iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

#[test]
fn stationery_preserves_all_50_prior_layers_6159_first_seen_ranks_and_phrase_endings() {
    let en = builtin("en").unwrap();
    let old = &en.word_layers()[..50];
    assert_eq!(old.last().unwrap().id, "daily_room_comfort");
    assert_eq!(old.iter().map(|l| l.next_words.len()).sum::<usize>(), 839);
    assert_eq!(old.iter().map(|l| l.sentences.len()).sum::<usize>(), 1642);
    assert_eq!(fnv(serde_json::to_vec(old).unwrap()), 0x2b54076deaad37da);
    let forms = indexed_occurrences(old);
    assert_eq!(forms.len(), 6159);
    assert_eq!(
        fnv(forms.iter().flat_map(|(word, rank)| {
            word.bytes().chain([0]).chain((*rank as u64).to_le_bytes())
        })),
        0xf89552dde388278a
    );
    assert!(
        NEW_FORMS
            .iter()
            .all(|new| forms.iter().all(|(old, _)| old != new))
    );
    let endings: Vec<_> = en.phrase_endings().collect();
    assert_eq!(endings.len(), 4);
    assert_eq!(
        fnv(serde_json::to_vec(&endings).unwrap()),
        0xfef0f04205b023c4
    );
}

#[test]
fn stationery_fixed_layer_adds_only_eight_forms_in_exact_first_occurrence_order() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 53);
    assert_eq!(en.next_words().count(), 857);
    assert_eq!(en.sentences().count(), 1678);
    let layer = &en.word_layers()[50];
    assert_eq!(layer.id, "daily_stationery");
    assert_eq!(
        layer.words.iter().map(String::as_str).collect::<Vec<_>>(),
        NEW_FORMS
    );
    assert_eq!(layer.next_words.len(), 6);
    for (i, (context, words)) in layer.next_words.iter().enumerate() {
        assert_eq!(context, CONTEXTS[i]);
        assert_eq!(
            words.iter().map(String::as_str).collect::<Vec<_>>(),
            WORDS[i]
        );
    }
    assert_eq!(
        layer
            .sentences
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        SENTENCES.into_iter().flatten().collect::<Vec<_>>()
    );
    let old = indexed_occurrences(&en.word_layers()[..50]);
    let all = indexed_occurrences(&en.word_layers()[..51]);
    assert_eq!(all.len(), 6167);
    assert_eq!(&all[..6159], old.as_slice());
    assert_eq!(
        all[6159..]
            .iter()
            .map(|(word, _)| word.as_str())
            .collect::<Vec<_>>(),
        NEW_FORMS
    );
    assert_eq!(
        all[6159..]
            .iter()
            .map(|(_, rank)| *rank)
            .collect::<Vec<_>>(),
        (18409..18417).collect::<Vec<_>>()
    );
    for word in NEW_FORMS {
        assert!(is_known_english_word(word), "{word}");
        let mut ime = engine(word);
        draft(&ime, word);
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        commit_once(&mut ime, word, CandidateKind::Word);
    }
    for (partial, word) in [
        ("pen", "pens"),
        ("penci", "pencil"),
        ("pencil", "pencils"),
        ("erase", "eraser"),
        ("rule", "ruler"),
        ("stick", "sticky"),
    ] {
        let ime = engine(partial);
        draft(&ime, partial);
        index(&ime, word, CandidateKind::Word);
    }
    // Bare pe/in are crowded and pen/rule already had pending/rule. Require
    // only the explicit partials above and new full forms, not arbitrary pages.
}

#[test]
fn stationery_twelve_specific_branches_keep_page_one_words_authors_and_exact_commits() {
    for (context, next, partial, sentence) in cases() {
        assert!(next.starts_with(partial) && next != partial);
        let word = format!("{context} {next}");
        for seed in [
            format!("{context} "),
            format!("{context} {partial}"),
            word.clone(),
        ] {
            choices(&seed, &word, sentence);
            assert_eq!(
                engine(&seed).prediction_status(),
                PredictionStatus::Disabled
            );
            raw_once(&seed);
        }
        let seed = format!("{context} {partial}");
        commit_once(&mut engine(&seed), &word, CandidateKind::Word);
        commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
    }
}

#[test]
fn stationery_preserves_case_smart_apostrophes_six_gaps_and_adopted_continuation() {
    for (seed, word, sentence) in [
        (
            "INSIDE MY PENCIL CASE I KEEP PEN",
            "INSIDE MY PENCIL CASE I KEEP PENS",
            "INSIDE MY PENCIL CASE I KEEP PENS for taking notes.",
        ),
        (
            "ONCE I’VE FINISHED WRITING I’LL PUT THE PE",
            "ONCE I’VE FINISHED WRITING I’LL PUT THE PEN",
            "ONCE I’VE FINISHED WRITING I’LL PUT THE PEN back in the drawer.",
        ),
        (
            "The ink in this pen has LEF",
            "The ink in this pen has LEFT",
            "The ink in this pen has LEFT a mark on my notebook.",
        ),
    ] {
        choices(seed, word, sentence);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        raw_once(seed);
    }
    for (context, next, partial, sentence) in cases() {
        let tail = sentence.strip_prefix(&format!("{context} {next}")).unwrap();
        for gap in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
            let seed = format!("{gap}{}{gap}{partial}", context.replace(' ', gap));
            let word = format!("{gap}{}{gap}{next}", context.replace(' ', gap));
            choices(&seed, &word, &format!("{word}{tail}"));
            raw_once(&seed);
        }
    }
    for (i, partial_tail, full_tail, following) in [
        (0, "fo", "for", " "),
        (1, "o", "on", " "),
        (2, "ou", "out", ", "),
        (3, "th", "the", " "),
        (4, "enoug", "enough", " "),
        (5, "bac", "back", " "),
    ] {
        let (context, next, partial, sentence) =
            (CONTEXTS[i], WORDS[i][0], PARTIALS[i][0], SENTENCES[i][0]);
        let word = format!("{context} {next}");
        let rest = sentence.strip_prefix(&format!("{word} ")).unwrap();
        assert!(rest.starts_with(&format!("{full_tail}{following}")));
        for prefix in [String::new(), "前文😀。  ".repeat(40)] {
            let seed = format!("{prefix}{context} {partial}");
            let adopted = format!("{prefix}{word}");
            let mut ime = engine(&seed);
            ime.select_candidate(index(&ime, &adopted, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(adopted.as_str()));
            assert_eq!(ime.snapshot().seed_text, seed);
            assert!(ime.snapshot().committed_text.is_empty());
            ime.seed(&adopted);
            draft(&ime, &adopted);
            let expected = format!("{adopted}  {rest}");
            for progress in [
                "  ".to_owned(),
                format!("  {partial_tail}"),
                format!("  {full_tail}"),
                format!("  {partial_tail}"),
                "  ".to_owned(),
            ] {
                let continued = format!("{adopted}{progress}");
                ime.seed(&continued);
                draft(&ime, &continued);
                index(&ime, &expected, CandidateKind::Sentence);
            }
            commit_once(&mut ime, &expected, CandidateKind::Sentence);
            raw_once(&seed);
        }
    }
}

#[test]
fn stationery_keeps_old_notebook_paper_controls_and_protected_line_and_unicode_boundaries() {
    for (seed, word, sentence) in [
        (
            "could i borrow your noteb",
            "could i borrow your notebook",
            "could i borrow your notebook for a moment?",
        ),
        (
            "may i use your note",
            "may i use your notebook",
            "may i use your notebook to take a few notes?",
        ),
        (
            "where did you put the scis",
            "where did you put the scissors",
            "where did you put the scissors?",
        ),
        (
            "could you bring me some pape",
            "could you bring me some paper",
            "could you bring me some paper from the desk?",
        ),
    ] {
        choices(seed, word, sentence);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
    for (context, _, partial, sentence) in cases() {
        let (left, right) = context.rsplit_once(' ').unwrap();
        for separator in [
            "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            let seed = format!("{left}{separator}{right} {partial}");
            let forbidden = format!(
                "{left}{separator}{}",
                sentence.strip_prefix(&format!("{left} ")).unwrap()
            );
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
            raw_once(&seed);
        }
        for prefix in ["https://('", "file_name_('", "unknownprefix", "前文😀"] {
            let seed = format!("{prefix}{context} {partial}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|c| c.text == format!("{prefix}{sentence}"))
            );
            raw_once(&seed);
        }
    }
    for seed in [
        "未完成🙂 pencil",
        "https://example.invalid/penci",
        "pen_erase",
        "xyzsticky",
    ] {
        raw_once(seed);
    }
    let seed = "the eraser next to my notebook is clea";
    commit_once(&mut engine(seed), SENTENCES[4][1], CandidateKind::Sentence);
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "stationery-synthetic-missing"
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
fn stationery_complete_local_pools_survive_pending_unavailable_timeout_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::HttpStatus(503)),
        Some(LlmProviderError::InvalidResponse),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (context, next, partial, sentence) in cases() {
            let seed = format!("{context} {partial}");
            let mut ime = engine(&seed);
            let local = ime.candidates().to_vec();
            ime.configure_prediction(Some(Arc::new(MissingProvider(error.clone()))));
            ime.seed(&seed);
            assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
            draft(&ime, &seed);
            index(&ime, &format!("{context} {next}"), CandidateKind::Word);
            index(&ime, sentence, CandidateKind::Sentence);
            assert_eq!(ime.candidates(), local);
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
            draft(&ime, &seed);
            assert_eq!(ime.candidates(), local);
            commit_once(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

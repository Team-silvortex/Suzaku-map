//! Finite physical-reading fallback, not book content or arbitrary short-prefix coverage.
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

const NEW_FORMS: [&str; 4] = ["bookmark", "pages", "paperback", "hardcover"];
const CONTEXTS: [&str; 6] = [
    "this bookmark will help me find the",
    "these pages are hard to",
    "before closing this book i'll finish the",
    "the paperback on my desk is",
    "the hardcover on this shelf is",
    "after reading these pages i'll",
];
const WORDS: [[&str; 2]; 6] = [
    ["page", "chapter"],
    ["read", "turn"],
    ["page", "chapter"],
    ["open", "closed"],
    ["mine", "yours"],
    ["put", "take"],
];
const PARTIALS: [[&str; 2]; 6] = [
    ["pag", "chapt"],
    ["rea", "tur"],
    ["pag", "chapt"],
    ["ope", "close"],
    ["min", "your"],
    ["pu", "tak"],
];
const SENTENCES: [[&str; 2]; 6] = [
    [
        "this bookmark will help me find the page where i stopped.",
        "this bookmark will help me find the chapter i want to read again.",
    ],
    [
        "these pages are hard to read in this light.",
        "these pages are hard to turn with one hand.",
    ],
    [
        "before closing this book i'll finish the page i'm reading.",
        "before closing this book i'll finish the chapter and leave a bookmark.",
    ],
    [
        "the paperback on my desk is open at the page i need.",
        "the paperback on my desk is closed with a bookmark inside.",
    ],
    [
        "the hardcover on this shelf is mine and you can borrow it.",
        "the hardcover on this shelf is yours and i'll bring it back tomorrow.",
    ],
    [
        "after reading these pages i'll put the book back on the shelf.",
        "after reading these pages i'll take a break before the next chapter.",
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
fn reading_preserves_all_51_prior_layers_6167_first_seen_ranks_and_phrase_endings() {
    let en = builtin("en").unwrap();
    let old = &en.word_layers()[..51];
    assert_eq!(old.last().unwrap().id, "daily_stationery");
    assert_eq!(old.iter().map(|l| l.next_words.len()).sum::<usize>(), 845);
    assert_eq!(old.iter().map(|l| l.sentences.len()).sum::<usize>(), 1654);
    assert_eq!(fnv(serde_json::to_vec(old).unwrap()), 0x74d9d3dcaf17f26c);
    let forms = indexed_occurrences(old);
    assert_eq!(forms.len(), 6167);
    assert_eq!(
        fnv(forms.iter().flat_map(|(word, rank)| {
            word.bytes().chain([0]).chain((*rank as u64).to_le_bytes())
        })),
        0xcbd20cf3aa266fd3
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
fn reading_fixed_layer_adds_only_four_forms_in_exact_first_occurrence_order() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 61);
    assert_eq!(en.next_words().count(), 894);
    assert_eq!(en.sentences().count(), 1752);
    let layer = &en.word_layers()[51];
    assert_eq!(layer.id, "daily_reading");
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
    let old = indexed_occurrences(&en.word_layers()[..51]);
    let all = indexed_occurrences(&en.word_layers()[..52]);
    assert_eq!(all.len(), 6171);
    assert_eq!(&all[..6167], old.as_slice());
    assert_eq!(
        all[6167..]
            .iter()
            .map(|(word, _)| word.as_str())
            .collect::<Vec<_>>(),
        NEW_FORMS
    );
    assert_eq!(
        all[6167..]
            .iter()
            .map(|(_, rank)| *rank)
            .collect::<Vec<_>>(),
        (18581..18585).collect::<Vec<_>>()
    );
    for word in NEW_FORMS {
        assert!(is_known_english_word(word), "{word}");
        let mut ime = engine(word);
        draft(&ime, word);
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        commit_once(&mut ime, word, CandidateKind::Word);
    }
    for (partial, word) in [
        ("bookmar", "bookmark"),
        ("page", "pages"),
        ("paperb", "paperback"),
        ("hardcov", "hardcover"),
    ] {
        let ime = engine(partial);
        draft(&ime, partial);
        index(&ime, word, CandidateKind::Word);
    }
    // Bookmark already exists in the optional internet pack, not the old
    // builtin window. Bare book/paper/hard are not promised arbitrary pages.
}

#[test]
fn reading_twelve_specific_branches_keep_page_one_words_authors_and_exact_commits() {
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
fn reading_preserves_case_smart_apostrophes_six_gaps_and_adopted_continuation() {
    for (seed, word, sentence) in [
        (
            "THESE PAGES ARE HARD TO TUR",
            "THESE PAGES ARE HARD TO TURN",
            "THESE PAGES ARE HARD TO TURN with one hand.",
        ),
        (
            "BEFORE CLOSING THIS BOOK I’LL FINISH THE CHAPT",
            "BEFORE CLOSING THIS BOOK I’LL FINISH THE CHAPTER",
            "BEFORE CLOSING THIS BOOK I’LL FINISH THE CHAPTER and leave a bookmark.",
        ),
        (
            "The hardcover on this shelf is YOUR",
            "The hardcover on this shelf is YOURS",
            "The hardcover on this shelf is YOURS and i'll bring it back tomorrow.",
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
        (0, "whe", "where", " "),
        (1, "i", "in", " "),
        (2, "i", "i'm", " "),
        (3, "a", "at", " "),
        (4, "an", "and", " "),
        (5, "th", "the", " "),
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
fn reading_keeps_old_leisure_notebook_controls_and_protected_line_and_unicode_boundaries() {
    for (seed, word, sentence) in [
        (
            "i've been reading thi",
            "i've been reading this",
            "i've been reading this book lately.",
        ),
        (
            "we could spend the afternoon rea",
            "we could spend the afternoon reading",
            "we could spend the afternoon reading at home.",
        ),
        (
            "could i borrow your noteb",
            "could i borrow your notebook",
            "could i borrow your notebook for a moment?",
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
        "未完成🙂 bookmark",
        "https://example.invalid/paperb",
        "page_hardcov",
        "xyzpaperback",
    ] {
        raw_once(seed);
    }
    let seed = "these pages are hard to tur";
    commit_once(&mut engine(seed), SENTENCES[1][1], CandidateKind::Sentence);
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "reading-synthetic-missing"
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
fn reading_complete_local_pools_survive_pending_unavailable_timeout_and_empty_models() {
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

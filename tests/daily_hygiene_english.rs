//! Finite everyday washcare/paper-item fallback, not health advice or stock tracking.
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
    "paper",
    "soap",
    "shampoo",
    "toothpaste",
    "tissue",
    "tissues",
    "conditioner",
    "mouthwash",
];
const CONTEXTS: [&str; 6] = [
    "could you bring me some",
    "we've run out of",
    "i've left the",
    "could you hand me a",
    "there's another bottle of",
    "please check if we need",
];
const WORDS: [[&str; 2]; 6] = [
    ["paper", "tissues"],
    ["soap", "toothpaste"],
    ["shampoo", "conditioner"],
    ["tissue", "towel"],
    ["shampoo", "mouthwash"],
    ["toothpaste", "conditioner"],
];
const PARTIALS: [[&str; 2]; 6] = [
    ["pape", "tissue"],
    ["soa", "toothpast"],
    ["shampo", "conditione"],
    ["tissu", "towe"],
    ["shampo", "mouthwas"],
    ["toothpast", "conditione"],
];
const SENTENCES: [[&str; 2]; 6] = [
    [
        "could you bring me some paper from the desk?",
        "could you bring me some tissues from the bathroom?",
    ],
    [
        "we've run out of soap by the sink.",
        "we've run out of toothpaste so i'll add it to the list.",
    ],
    [
        "i've left the shampoo on the bathroom shelf.",
        "i've left the conditioner next to the shampoo.",
    ],
    [
        "could you hand me a tissue from that box?",
        "could you hand me a towel from the shelf?",
    ],
    [
        "there's another bottle of shampoo in the cupboard.",
        "there's another bottle of mouthwash on the bathroom shelf.",
    ],
    [
        "please check if we need toothpaste before you go out.",
        "please check if we need conditioner before buying more.",
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
fn hygiene_preserves_all_47_prior_layers_first_seen_ranks_and_phrase_endings() {
    let en = builtin("en").unwrap();
    let old = &en.word_layers()[..47];
    assert_eq!(old.last().unwrap().id, "daily_restock");
    assert_eq!(old.iter().map(|l| l.next_words.len()).sum::<usize>(), 821);
    assert_eq!(old.iter().map(|l| l.sentences.len()).sum::<usize>(), 1606);
    assert_eq!(fnv(serde_json::to_vec(old).unwrap()), 0x915ae24252e5bfec);
    let forms = indexed_occurrences(old);
    assert_eq!(forms.len(), 6143);
    assert_eq!(
        fnv(forms.iter().flat_map(|(word, rank)| {
            word.bytes().chain([0]).chain((*rank as u64).to_le_bytes())
        })),
        0x032534d49a43f218
    );
    // Absence is asserted only in the frozen old window, never in current data.
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
fn hygiene_fixed_layer_adds_exactly_eight_forms_and_recognizes_complete_words() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 946);
    assert_eq!(en.sentences().count(), 1856);
    let layer = &en.word_layers()[47];
    assert_eq!(layer.id, "daily_hygiene");
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
    let old = indexed_occurrences(&en.word_layers()[..47]);
    let all = indexed_occurrences(&en.word_layers()[..48]);
    assert_eq!(all.len(), 6151);
    assert_eq!(&all[..6143], old.as_slice());
    assert_eq!(
        all[6143..]
            .iter()
            .map(|(w, _)| w.as_str())
            .collect::<Vec<_>>(),
        NEW_FORMS
    );
    for word in NEW_FORMS {
        assert!(is_known_english_word(word), "{word}");
        let ime = engine(word);
        draft(&ime, word);
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        commit_once(&mut engine(word), word, CandidateKind::Word);
    }
    for (partial, word) in [
        ("pape", "paper"),
        ("shampo", "shampoo"),
        ("toothpast", "toothpaste"),
        ("tissu", "tissue"),
        ("tissue", "tissues"),
        ("conditione", "conditioner"),
        ("mouthwas", "mouthwash"),
    ] {
        let ime = engine(partial);
        draft(&ime, partial);
        index(&ime, word, CandidateKind::Word);
    }
    // soa already has four older soak forms; require soap through its real context.
    choices(
        "we've run out of soa",
        "we've run out of soap",
        SENTENCES[1][0],
    );
}

#[test]
fn hygiene_twelve_fixed_branches_offer_page_one_words_authors_and_exact_commits() {
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
fn hygiene_case_smart_apostrophes_horizontal_space_and_adopted_progress_stay_literal() {
    for (seed, word, sentence) in [
        (
            "We’ve run out of TOOTHPAST",
            "We’ve run out of TOOTHPASTE",
            "We’ve run out of TOOTHPASTE so i'll add it to the list.",
        ),
        (
            "I’ve left the CONDITIONE",
            "I’ve left the CONDITIONER",
            "I’ve left the CONDITIONER next to the shampoo.",
        ),
        (
            "COULD YOU BRING ME SOME TISSUE",
            "COULD YOU BRING ME SOME TISSUES",
            "COULD YOU BRING ME SOME TISSUES from the bathroom?",
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
    for (i, branch, partial_tail, full_tail) in [
        (0, 0, "fro", "from"),
        (1, 1, "s", "so"),
        (2, 1, "nex", "next"),
        (3, 0, "fro", "from"),
        (4, 1, "o", "on"),
        (5, 1, "befor", "before"),
    ] {
        let (context, next, partial, sentence) = (
            CONTEXTS[i],
            WORDS[i][branch],
            PARTIALS[i][branch],
            SENTENCES[i][branch],
        );
        let word = format!("{context} {next}");
        let tail = sentence.strip_prefix(&word).unwrap();
        assert!(tail.starts_with(&format!(" {full_tail} ")));
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
            let expected = format!("{adopted}  {}", tail.trim_start());
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
fn hygiene_keeps_old_controls_protected_tokens_seven_line_boundaries_and_raw_undo() {
    for (seed, word, sentence) in [
        (
            "i need to wash my han",
            "i need to wash my hands",
            "i need to wash my hands before we leave.",
        ),
        (
            "where can i refill my bottl",
            "where can i refill my bottle",
            "where can i refill my bottle?",
        ),
        (
            "we're nearly out of detergen",
            "we're nearly out of detergent",
            "we're nearly out of detergent so i'll add it to the list.",
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
        "未完成🙂 paper",
        "https://example.invalid/tissu",
        "paper_tissu",
        "xyztissue",
    ] {
        raw_once(seed);
    }
    let seed = "could you hand me a tissu";
    commit_once(&mut engine(seed), SENTENCES[3][0], CandidateKind::Sentence);
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "hygiene-synthetic-missing"
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
fn hygiene_complete_local_pools_survive_unavailable_timeout_and_empty_models() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (context, next, partial, sentence) in cases() {
            let seed = format!("{context} {partial}");
            let mut ime = engine(&seed);
            let local = ime.candidates().to_vec();
            ime.configure_prediction(Some(Arc::new(MissingProvider(error.clone()))));
            ime.seed(&seed);
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

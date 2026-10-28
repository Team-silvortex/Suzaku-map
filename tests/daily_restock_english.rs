//! Fixed household restocking utterances, not inventory tracking or shopping actions.
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
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{WordLayer, builtin},
};

const CONTEXTS: [&str; 6] = [
    "we're nearly out of",
    "could you add these to the",
    "do we have any spare",
    "there's an unopened",
    "please use the open",
    "please tell me when we're",
];
const WORDS: [[&str; 2]; 6] = [
    ["detergent", "batteries"],
    ["list", "basket"],
    ["batteries", "lightbulbs"],
    ["bottle", "packet"],
    ["bottle", "packet"],
    ["nearly", "out"],
];
const PARTIALS: [[&str; 2]; 6] = [
    ["detergen", "batterie"],
    ["lis", "baske"],
    ["batterie", "lightbulb"],
    ["bottl", "packe"],
    ["bottl", "packe"],
    ["nearl", "ou"],
];
const SENTENCES: [[&str; 2]; 6] = [
    [
        "we're nearly out of detergent so i'll add it to the list.",
        "we're nearly out of batteries so please check the drawer first.",
    ],
    [
        "could you add these to the list before we go shopping?",
        "could you add these to the basket after checking what we have at home?",
    ],
    [
        "do we have any spare batteries in the drawer?",
        "do we have any spare lightbulbs or should i buy another pack?",
    ],
    [
        "there's an unopened bottle of detergent in the cupboard.",
        "there's an unopened packet of sponges under the sink.",
    ],
    [
        "please use the open bottle before you start a new one.",
        "please use the open packet before opening another one.",
    ],
    [
        "please tell me when we're nearly out so i can get some more.",
        "please tell me when we're out of detergent and i'll add it to the list.",
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
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local && c.text.starts_with(seed))
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
fn indexed_forms(layers: &[WordLayer]) -> Vec<String> {
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
        .map(|word| word.replace('’', "'").to_ascii_lowercase())
        .filter(|word| seen.insert(word.clone()))
        .collect()
}

#[test]
fn restock_layer_has_fixed_records_without_new_or_reordered_index_forms() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 946);
    assert_eq!(en.sentences().count(), 1856);
    let previous = &en.word_layers()[..46];
    assert_eq!(
        previous.iter().map(|l| l.next_words.len()).sum::<usize>(),
        815
    );
    assert_eq!(
        previous.iter().map(|l| l.sentences.len()).sum::<usize>(),
        1594
    );
    let layer = &en.word_layers()[46];
    assert_eq!(layer.id, "daily_restock");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 6);
    for (i, (context, words)) in layer.next_words.iter().enumerate() {
        assert_eq!(context, CONTEXTS[i]);
        assert_eq!(
            words.iter().map(String::as_str).collect::<Vec<_>>(),
            WORDS[i]
        );
        assert!(
            previous
                .iter()
                .all(|l| l.next_words.iter().all(|(old, _)| old != context))
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
    assert!(
        layer
            .sentences
            .iter()
            .all(|sentence| previous.iter().all(|old| !old.sentences.contains(sentence)))
    );
    let old = indexed_forms(previous);
    assert_eq!(old.len(), 6143);
    let current = indexed_forms(en.word_layers());
    assert_eq!(current.len(), 6216);
    assert_eq!(&current[..6143], old.as_slice());
}

#[test]
fn restock_fixed_branches_offer_first_page_word_sentence_and_raw_once_only_commit() {
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
fn restock_keeps_case_curly_apostrophes_and_six_horizontal_separators() {
    for (seed, word, sentence) in [
        (
            "We’re nearly out of DETERGEN",
            "We’re nearly out of DETERGENT",
            "We’re nearly out of DETERGENT so i'll add it to the list.",
        ),
        (
            "There’s an unopened PACKE",
            "There’s an unopened PACKET",
            "There’s an unopened PACKET of sponges under the sink.",
        ),
        (
            "DO WE HAVE ANY SPARE LIGHTBULB",
            "DO WE HAVE ANY SPARE LIGHTBULBS",
            "DO WE HAVE ANY SPARE LIGHTBULBS or should i buy another pack?",
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
            let sentence = format!("{word}{tail}");
            choices(&seed, &word, &sentence);
            commit_once(&mut engine(&seed), &sentence, CandidateKind::Sentence);
            raw_once(&seed);
        }
    }
}

#[test]
fn restock_word_adoption_space_tail_progress_and_reseeding_do_not_commit() {
    let tails = [
        (0, "s", "so"),
        (0, "befor", "before"),
        (0, "i", "in"),
        (1, "o", "of"),
        (1, "befor", "before"),
        (0, "ou", "out"),
    ];
    for (i, (branch, partial_tail, full_tail)) in tails.into_iter().enumerate() {
        let (context, next, partial, sentence) = (
            CONTEXTS[i],
            WORDS[i][branch],
            PARTIALS[i][branch],
            SENTENCES[i][branch],
        );
        let word = format!("{context} {next}");
        let tail = sentence.strip_prefix(&word).unwrap();
        assert!(tail.starts_with(&format!(" {full_tail} ")));
        for prefix in [String::new(), "前文😀。  ".repeat(100)] {
            let seed = format!("{prefix}{context} {partial}");
            let adopted = format!("{prefix}{word}");
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            ime.select_candidate(index(&ime, &adopted, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(adopted.as_str()));
            assert_eq!(ime.snapshot().seed_text, seed);
            assert!(ime.snapshot().committed_text.is_empty());
            // Host-like adoption/reseed, never simulated physical keyboard input.
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
fn restock_preserves_old_controls_and_rejects_unknown_protected_or_cross_line_phrases() {
    for (seed, word, sentence) in [
        (
            "let's make a shoppin",
            "let's make a shopping",
            "let's make a shopping list.",
        ),
        (
            "there's a spare char",
            "there's a spare charger",
            "there's a spare charger on the table.",
        ),
        (
            "where can i refill my bottl",
            "where can i refill my bottle",
            "where can i refill my bottle?",
        ),
    ] {
        choices(seed, word, sentence);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        raw_once(seed);
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
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "restock-synthetic-missing"
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
fn restock_exact_local_pools_survive_unavailable_timeout_and_empty_model_results() {
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

//! Finite kitchen-task communication fallback, not recipes or arbitrary short-prefix coverage.
use ring::digest::{SHA256, digest};
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
    "bowls", "spoons", "forks", "apron", "cutting", "rack", "tray", "lids",
];
const CONTEXTS: [&str; 6] = [
    "on the cutting board i've left the",
    "the clean bowls are ready for",
    "next to the plates i've put the",
    "my apron is hanging near the",
    "the drying rack still has the",
    "these lids belong to the",
];
const WORDS: [[&str; 2]; 6] = [
    ["vegetables", "bread"],
    ["soup", "rice"],
    ["spoons", "forks"],
    ["door", "sink"],
    ["cups", "bowls"],
    ["containers", "pots"],
];
const PARTIALS: [[&str; 2]; 6] = [
    ["vegetabl", "brea"],
    ["sou", "ric"],
    ["spoo", "fork"],
    ["doo", "sin"],
    ["cup", "bowl"],
    ["container", "pot"],
];
const SENTENCES: [[&str; 2]; 6] = [
    [
        "on the cutting board i've left the vegetables you asked me to bring.",
        "on the cutting board i've left the bread for our lunch.",
    ],
    [
        "the clean bowls are ready for soup when you bring it over.",
        "the clean bowls are ready for rice and i'll carry them to the table.",
    ],
    [
        "next to the plates i've put the spoons for everyone.",
        "next to the plates i've put the forks from the kitchen drawer.",
    ],
    [
        "my apron is hanging near the door where i can find it.",
        "my apron is hanging near the sink with the clean towels.",
    ],
    [
        "the drying rack still has the cups we used at lunch.",
        "the drying rack still has the bowls from our lunch.",
    ],
    [
        "these lids belong to the containers on the tray.",
        "these lids belong to the pots in the cupboard.",
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
fn kitchen_preserves_all_53_prior_layers_6179_first_seen_ranks_endings_and_raw_bytes() {
    let en = builtin("en").unwrap();
    let old = &en.word_layers()[..53];
    assert_eq!(old.last().unwrap().id, "daily_plants");
    assert_eq!(old.iter().map(|l| l.next_words.len()).sum::<usize>(), 857);
    assert_eq!(old.iter().map(|l| l.sentences.len()).sum::<usize>(), 1678);
    assert_eq!(fnv(serde_json::to_vec(old).unwrap()), 0xf7cfcf0516597518);
    let forms = indexed_occurrences(old);
    assert_eq!(forms.len(), 6179);
    assert_eq!(
        fnv(forms.iter().flat_map(|(word, rank)| {
            word.bytes().chain([0]).chain((*rank as u64).to_le_bytes())
        })),
        0x152328ca5a235710
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
    let mut raw = include_str!("../data/lexicons/en.json").to_owned();
    if en.word_layers().len() > 53 {
        let first = ",\n    {\n      \"id\": \"daily_kitchen\",";
        let after = "\n  ],\n  \"phrase_endings\": [";
        assert_eq!(raw.matches(first).count(), 1);
        assert_eq!(raw.matches(after).count(), 1);
        let (start, end) = (raw.find(first).unwrap(), raw.find(after).unwrap());
        assert!(start < end);
        raw.replace_range(start..end, "");
    }
    let raw_sha = digest(&SHA256, raw.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        raw_sha,
        "3102776d198a40a6ae64829666c8ba74e457ef2c4fd57e134c9ac7c3b76705cc"
    );
}

#[test]
fn kitchen_fixed_layer_adds_only_eight_forms_in_exact_first_occurrence_order() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 55);
    assert_eq!(en.next_words().count(), 867);
    assert_eq!(en.sentences().count(), 1698);
    let layer = &en.word_layers()[53];
    assert_eq!(layer.id, "daily_kitchen");
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
    let old = indexed_occurrences(&en.word_layers()[..53]);
    let all = indexed_occurrences(&en.word_layers()[..54]);
    assert_eq!(all.len(), 6187);
    assert_eq!(&all[..6179], old.as_slice());
    assert_eq!(
        all[6179..]
            .iter()
            .map(|(word, _)| word.as_str())
            .collect::<Vec<_>>(),
        NEW_FORMS
    );
    assert_eq!(
        all[6179..]
            .iter()
            .map(|(_, rank)| *rank)
            .collect::<Vec<_>>(),
        (18888..18896).collect::<Vec<_>>()
    );
    for word in NEW_FORMS {
        assert!(is_known_english_word(word), "{word}");
        let mut ime = engine(word);
        draft(&ime, word);
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        commit_once(&mut ime, word, CandidateKind::Word);
    }
    for (partial, word) in [
        ("bowl", "bowls"),
        ("spoo", "spoons"),
        ("fork", "forks"),
        ("apro", "apron"),
        ("cutt", "cutting"),
        ("lid", "lids"),
    ] {
        let ime = engine(partial);
        draft(&ime, partial);
        index(&ime, word, CandidateKind::Word);
    }
    // Bowls/spoons already exist in the optional cooking pack, not the old
    // builtin. The crowded bare tra/rac roots are not promised first-page choices.
}

#[test]
fn kitchen_twelve_specific_branches_keep_page_one_words_authors_and_exact_commits() {
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
fn kitchen_preserves_case_smart_apostrophes_six_gaps_and_adopted_continuation() {
    for (seed, word, sentence) in [
        (
            "ON THE CUTTING BOARD I’VE LEFT THE BREA",
            "ON THE CUTTING BOARD I’VE LEFT THE BREAD",
            "ON THE CUTTING BOARD I’VE LEFT THE BREAD for our lunch.",
        ),
        (
            "Next to the plates I’VE PUT THE SPOO",
            "Next to the plates I’VE PUT THE SPOONS",
            "Next to the plates I’VE PUT THE SPOONS for everyone.",
        ),
        (
            "My apron is hanging near the SIN",
            "My apron is hanging near the SINK",
            "My apron is hanging near the SINK with the clean towels.",
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
        (0, "yo", "you", " "),
        (1, "whe", "when", " "),
        (2, "fo", "for", " "),
        (3, "whe", "where", " "),
        (4, "w", "we", " "),
        (5, "o", "on", " "),
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
fn kitchen_keeps_old_household_mealtime_controls_and_protected_line_and_unicode_boundaries() {
    for word in [
        "bowl", "spoon", "fork", "knife", "board", "napkins", "drying", "pots",
    ] {
        assert!(is_known_english_word(word), "{word}");
        raw_once(word);
    }
    for (seed, word, sentence) in [
        (
            "i can wash the di",
            "i can wash the dishes",
            "i can wash the dishes after dinner.",
        ),
        (
            "please wipe the cou",
            "please wipe the counter",
            "please wipe the counter with a clean towel.",
        ),
        (
            "please put a spo",
            "please put a spoon",
            "please put a spoon next to each bowl.",
        ),
        (
            "please put the leftovers in the ref",
            "please put the leftovers in the refrigerator",
            "please put the leftovers in the refrigerator.",
        ),
        (
            "i'll clear the di",
            "i'll clear the dishes",
            "i'll clear the dishes after we eat.",
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
        "未完成🙂 apron",
        "https://example.invalid/cutt",
        "bowl_spoo",
        "xyzcutting",
    ] {
        raw_once(seed);
    }
    let seed = "the drying rack still has the bowl";
    commit_once(&mut engine(seed), SENTENCES[4][1], CandidateKind::Sentence);
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "kitchen-synthetic-missing"
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
fn kitchen_complete_local_pools_survive_pending_unavailable_timeout_and_empty_models() {
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

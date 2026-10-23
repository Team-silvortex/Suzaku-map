//! Finite dressing/fit/accessory examples, not shopping, inventory or sizing services.
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
    "my shirt feels",
    "the sleeves on this shirt are",
    "the zipper on my jacket is",
    "before leaving i'll adjust my",
    "my glasses are inside the",
    "i'll change into a",
];
const WORDS: [[&str; 2]; 6] = [
    ["tight", "loose"],
    ["long", "short"],
    ["stuck", "open"],
    ["belt", "scarf"],
    ["case", "bag"],
    ["shirt", "sweater"],
];
const PARTIALS: [[&str; 2]; 6] = [
    ["tigh", "loos"],
    ["lon", "shor"],
    ["stuc", "ope"],
    ["bel", "scar"],
    ["cas", "ba"],
    ["shir", "sweate"],
];
const SENTENCES: [[&str; 2]; 6] = [
    [
        "my shirt feels tight when i sit down.",
        "my shirt feels loose, so i'll try a smaller one.",
    ],
    [
        "the sleeves on this shirt are long, so i'll fold them back.",
        "the sleeves on this shirt are short, so i'll wear a jacket over it.",
    ],
    [
        "the zipper on my jacket is stuck near the bottom.",
        "the zipper on my jacket is open, so i'll close it before we leave.",
    ],
    [
        "before leaving i'll adjust my belt so it fits better.",
        "before leaving i'll adjust my scarf to keep it in place.",
    ],
    [
        "my glasses are inside the case on the shelf.",
        "my glasses are inside the bag next to my notebook.",
    ],
    [
        "i'll change into a shirt before we go out.",
        "i'll change into a sweater because it's cold outside.",
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
fn dressing_preserves_all_48_prior_layers_6151_first_seen_ranks_and_phrase_endings() {
    let en = builtin("en").unwrap();
    let old = &en.word_layers()[..48];
    assert_eq!(old.last().unwrap().id, "daily_hygiene");
    assert_eq!(old.iter().map(|l| l.next_words.len()).sum::<usize>(), 827);
    assert_eq!(old.iter().map(|l| l.sentences.len()).sum::<usize>(), 1618);
    assert_eq!(fnv(serde_json::to_vec(old).unwrap()), 0x9f85f0b6a02ee08d);
    let forms = indexed_occurrences(old);
    assert_eq!(forms.len(), 6151);
    assert_eq!(
        fnv(forms.iter().flat_map(|(word, rank)| {
            word.bytes().chain([0]).chain((*rank as u64).to_le_bytes())
        })),
        0xa8e31d0a8a1cd564
    );
    let endings: Vec<_> = en.phrase_endings().collect();
    assert_eq!(endings.len(), 4);
    assert_eq!(
        fnv(serde_json::to_vec(&endings).unwrap()),
        0xfef0f04205b023c4
    );
}

#[test]
fn dressing_fixed_layer_has_six_contexts_twelve_authors_and_no_new_indexed_forms() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 53);
    assert_eq!(en.next_words().count(), 857);
    assert_eq!(en.sentences().count(), 1678);
    let layer = &en.word_layers()[48];
    assert_eq!(layer.id, "daily_dressing");
    assert!(layer.words.is_empty());
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
    let old = indexed_occurrences(&en.word_layers()[..48]);
    let all = indexed_occurrences(&en.word_layers()[..49]);
    assert_eq!(all.len(), 6151);
    assert_eq!(all, old, "retain every prior word and its first-seen rank");
}

#[test]
fn dressing_twelve_specific_branches_keep_page_one_words_authors_and_exact_commits() {
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
fn dressing_keeps_case_smart_apostrophes_horizontal_space_and_adopted_progress_literal() {
    for (seed, word, sentence) in [
        (
            "MY SHIRT FEELS TIGH",
            "MY SHIRT FEELS TIGHT",
            "MY SHIRT FEELS TIGHT when i sit down.",
        ),
        (
            "before leaving I’ll adjust my SCAR",
            "before leaving I’ll adjust my SCARF",
            "before leaving I’ll adjust my SCARF to keep it in place.",
        ),
        (
            "I’LL change into a SWEATE",
            "I’LL change into a SWEATER",
            "I’LL change into a SWEATER because it's cold outside.",
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
    // Sleeve authors start with a comma: retain it before typing Space rather
    // than pretend the authored punctuation can be dropped on adoption.
    for (i, separator, partial_tail, full_tail) in [
        (0, "  ", "whe", "when"),
        (1, ",  ", "s", "so"),
        (2, "  ", "nea", "near"),
        (3, "  ", "s", "so"),
        (4, "  ", "o", "on"),
        (5, "  ", "befor", "before"),
    ] {
        let (context, next, partial, sentence) =
            (CONTEXTS[i], WORDS[i][0], PARTIALS[i][0], SENTENCES[i][0]);
        let word = format!("{context} {next}");
        let tail = sentence.strip_prefix(&word).unwrap();
        let rest = tail.strip_prefix(if i == 1 { ", " } else { " " }).unwrap();
        assert!(rest.starts_with(&format!("{full_tail} ")));
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
            let expected = format!("{adopted}{separator}{rest}");
            for progress in [
                separator.to_owned(),
                format!("{separator}{partial_tail}"),
                format!("{separator}{full_tail}"),
                format!("{separator}{partial_tail}"),
                separator.to_owned(),
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
fn dressing_keeps_old_carry_laundry_controls_and_protected_line_and_unicode_boundaries() {
    for (seed, word, sentence) in [
        (
            "please put on your jack",
            "please put on your jacket",
            "please put on your jacket before we leave.",
        ),
        (
            "i'll wear my coa",
            "i'll wear my coat",
            "i'll wear my coat because it's cold.",
        ),
        (
            "this shirt is still we",
            "this shirt is still wet",
            "this shirt is still wet, so i'll wear another one.",
        ),
        (
            "these socks don't fi",
            "these socks don't fit",
            "these socks don't fit me anymore.",
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
        "未完成🙂 scarf",
        "https://example.invalid/sweate",
        "shirt_tigh",
        "xyzsweater",
    ] {
        raw_once(seed);
    }
    let seed = "the zipper on my jacket is stuc";
    commit_once(&mut engine(seed), SENTENCES[2][0], CandidateKind::Sentence);
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "dressing-synthetic-missing"
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
fn dressing_complete_local_pools_survive_unavailable_timeout_and_empty_models() {
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

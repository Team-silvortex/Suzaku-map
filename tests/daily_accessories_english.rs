//! Finite accessory-location and carrying phrases, not charging or electrical instructions.
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

const NEW_FORMS: [&str; 6] = [
    "connector",
    "connectors",
    "cord",
    "cords",
    "pouch",
    "pouches",
];
const CONTEXTS: [&str; 6] = [
    "the spare charging cable is",
    "i put the adapter in the",
    "my earbuds are still in the",
    "the connectors for these cables are",
    "i left the spare cords",
    "these small pouches can hold",
];
const WORDS: [[&str; 2]; 6] = [
    ["here", "inside"],
    ["drawer", "pouch"],
    ["case", "pouch"],
    ["here", "there"],
    ["here", "upstairs"],
    ["cables", "adapters"],
];
const PARTIALS: [[&str; 2]; 6] = [
    ["her", "insid"],
    ["drawe", "pouc"],
    ["cas", "pouc"],
    ["her", "ther"],
    ["her", "upstair"],
    ["cable", "adapter"],
];
const SENTENCES: [[&str; 2]; 6] = [
    [
        "the spare charging cable is here on the desk.",
        "the spare charging cable is inside the small pouch.",
    ],
    [
        "i put the adapter in the drawer with the spare cables.",
        "i put the adapter in the pouch next to my earbuds.",
    ],
    [
        "my earbuds are still in the case next to my notebook.",
        "my earbuds are still in the pouch i brought yesterday.",
    ],
    [
        "the connectors for these cables are here in the top drawer.",
        "the connectors for these cables are there in the small box.",
    ],
    [
        "i left the spare cords here with the chargers.",
        "i left the spare cords upstairs in the desk drawer.",
    ],
    [
        "these small pouches can hold cables while we travel.",
        "these small pouches can hold adapters and the spare connector.",
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
fn accessories_preserves_56_prior_layers_6201_first_seen_ranks_endings_and_raw_bytes() {
    let en = builtin("en").unwrap();
    let old = &en.word_layers()[..56];
    assert_eq!(old.last().unwrap().id, "daily_recycling");
    assert_eq!(old.iter().map(|l| l.next_words.len()).sum::<usize>(), 873);
    assert_eq!(old.iter().map(|l| l.sentences.len()).sum::<usize>(), 1710);
    assert_eq!(fnv(serde_json::to_vec(old).unwrap()), 0xf899515f85e5e264);
    let forms = indexed_occurrences(old);
    assert_eq!(forms.len(), 6201);
    assert_eq!(
        fnv(forms.iter().flat_map(|(word, rank)| {
            word.bytes().chain([0]).chain((*rank as u64).to_le_bytes())
        })),
        0xd67a9a177b88722c
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
    if en.word_layers().len() > 56 {
        let first = ",\n    {\n      \"id\": \"daily_accessories\",";
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
        "9f144f170a3036ea97b95e7c3db56d28fbb1c56ca99ace2ed13024f04ac4e118"
    );
}

#[test]
fn accessories_fixed_layer_adds_six_forms_six_contexts_and_twelve_authors_only() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 61);
    assert_eq!(en.next_words().count(), 894);
    assert_eq!(en.sentences().count(), 1752);
    let layer = &en.word_layers()[56];
    assert_eq!(layer.id, "daily_accessories");
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
    let old = indexed_occurrences(&en.word_layers()[..56]);
    let all = indexed_occurrences(en.word_layers());
    assert_eq!(all.len(), 6213);
    assert_eq!(&all[..6201], old.as_slice());
    assert_eq!(
        all[6201..6207]
            .iter()
            .map(|(word, _)| word.as_str())
            .collect::<Vec<_>>(),
        NEW_FORMS
    );
    assert_eq!(
        all[6201..6207]
            .iter()
            .map(|(_, rank)| *rank)
            .collect::<Vec<_>>(),
        (19292..19298).collect::<Vec<_>>()
    );
    for word in NEW_FORMS {
        assert!(is_known_english_word(word), "{word}");
        let mut ime = engine(word);
        draft(&ime, word);
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        commit_once(&mut ime, word, CandidateKind::Word);
    }
    for (partial, word) in [
        ("connecto", "connector"),
        ("connecto", "connectors"),
        ("pouc", "pouch"),
        ("pouc", "pouches"),
    ] {
        let ime = engine(partial);
        draft(&ime, partial);
        index(&ime, word, CandidateKind::Word);
    }
    // cor/co are crowded roots: recognition does not promise new cords on their first page.
}

#[test]
fn accessories_twelve_branches_keep_first_page_words_sentences_and_exact_commits() {
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
            commit_once(&mut engine(&seed), &word, CandidateKind::Word);
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            raw_once(&seed);
        }
    }
}

#[test]
fn accessories_preserves_case_six_gaps_and_long_draft_adoption_reseed_continuation() {
    choices(
        "THE SPARE CHARGING CABLE IS HER",
        "THE SPARE CHARGING CABLE IS HERE",
        "THE SPARE CHARGING CABLE IS HERE on the desk.",
    );
    for (context, next, partial, sentence) in cases() {
        let tail = sentence.strip_prefix(&format!("{context} {next}")).unwrap();
        for gap in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
            let seed = format!("{gap}{}{gap}{partial}", context.replace(' ', gap));
            let word = format!("{gap}{}{gap}{next}", context.replace(' ', gap));
            choices(&seed, &word, &format!("{word}{tail}"));
            raw_once(&seed);
        }
    }
    for (i, tail_partial, tail_word) in [
        (0, "o", "on"),
        (1, "wit", "with"),
        (2, "nex", "next"),
        (3, "i", "in"),
        (4, "wit", "with"),
        (5, "whil", "while"),
    ] {
        let (context, next, partial, sentence) =
            (CONTEXTS[i], WORDS[i][0], PARTIALS[i][0], SENTENCES[i][0]);
        let word = format!("{context} {next}");
        let rest = sentence.strip_prefix(&format!("{word} ")).unwrap();
        assert!(rest.starts_with(&format!("{tail_word} ")));
        for prefix in [String::new(), "前文😀。  ".repeat(40)] {
            let seed = format!("{prefix}{context} {partial}");
            let adopted = format!("{prefix}{word}");
            let mut ime = engine(&seed);
            ime.select_candidate(index(&ime, &adopted, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(adopted.as_str()));
            assert_eq!(ime.snapshot().seed_text, seed);
            assert!(ime.snapshot().committed_text.is_empty());
            ime.seed(&adopted);
            let expected = format!("{adopted}  {rest}");
            for progress in [
                "  ".to_owned(),
                format!("  {tail_partial}"),
                format!("  {tail_word}"),
                format!("  {tail_partial}"),
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
fn accessories_keeps_old_device_choices_and_protects_lines_urls_identifiers_and_raw_input() {
    for word in [
        "charger", "chargers", "adapter", "adapters", "cable", "cables", "earbuds", "charging",
    ] {
        assert!(is_known_english_word(word), "{word}");
        raw_once(word);
    }
    for (seed, word, sentence) in [
        (
            "could i borrow your char",
            "could i borrow your charger",
            "could i borrow your charger until tonight?",
        ),
        (
            "i need to bring my char",
            "i need to bring my charger",
            "i need to bring my charger before we leave.",
        ),
        (
            "the charging cable is loo",
            "the charging cable is loose",
            "the charging cable is loose near the phone.",
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
        "未完成🙂 pouch",
        "https://example.invalid/connec",
        "adapter_pou",
        "xyzconnector",
    ] {
        raw_once(seed);
    }
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "accessories-synthetic-missing"
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
fn accessories_keeps_complete_local_pools_before_and_after_five_missing_model_results() {
    // Synthetic providers only; this does not validate a real model or service.
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
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

//! Finite bedding and bedtime tidying fallback, not sleep advice or arbitrary prefixes.
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
    "sheet", "sheets", "pillows", "bedside", "slippers", "pajamas",
];
const CONTEXTS: [&str; 4] = [
    "our spare bed still needs the",
    "these sheets can go in the",
    "my slippers and pajamas are ready",
    "the bedside lamp is on the",
];
const WORDS: [[&str; 2]; 4] = [
    ["sheet", "pillows"],
    ["cupboard", "basket"],
    ["upstairs", "here"],
    ["table", "shelf"],
];
const PARTIALS: [[&str; 2]; 4] = [
    ["shee", "pillo"],
    ["cupboar", "baske"],
    ["upstair", "her"],
    ["tabl", "shel"],
];
const SENTENCES: [[&str; 2]; 4] = [
    [
        "our spare bed still needs the sheet from the clean laundry.",
        "our spare bed still needs the pillows you left in the bedroom.",
    ],
    [
        "these sheets can go in the cupboard with the spare pillowcases.",
        "these sheets can go in the basket until i put them away.",
    ],
    [
        "my slippers and pajamas are ready upstairs in the bedroom.",
        "my slippers and pajamas are ready here on the chair.",
    ],
    [
        "the bedside lamp is on the table next to the book i'm reading.",
        "the bedside lamp is on the shelf where i left my glasses.",
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
fn bedding_preserves_all_54_prior_layers_6187_first_seen_ranks_endings_and_raw_bytes() {
    let en = builtin("en").unwrap();
    let old = &en.word_layers()[..54];
    assert_eq!(old.last().unwrap().id, "daily_kitchen");
    assert_eq!(old.iter().map(|l| l.next_words.len()).sum::<usize>(), 863);
    assert_eq!(old.iter().map(|l| l.sentences.len()).sum::<usize>(), 1690);
    assert_eq!(fnv(serde_json::to_vec(old).unwrap()), 0x1e20d120a31e1971);
    let forms = indexed_occurrences(old);
    assert_eq!(forms.len(), 6187);
    assert_eq!(
        fnv(forms.iter().flat_map(|(word, rank)| {
            word.bytes().chain([0]).chain((*rank as u64).to_le_bytes())
        })),
        0x2e9c06cdcb8cc90d
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
    if en.word_layers().len() > 54 {
        let first = ",\n    {\n      \"id\": \"daily_bedding\",";
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
        "b1ef05fa98967f14b2ccfecae03a19cf3f0fe33743638e9b8868793d11148de4"
    );
}

#[test]
fn bedding_fixed_layer_adds_only_six_forms_in_exact_first_occurrence_order() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 61);
    assert_eq!(en.next_words().count(), 894);
    assert_eq!(en.sentences().count(), 1752);
    let layer = &en.word_layers()[54];
    assert_eq!(layer.id, "daily_bedding");
    assert_eq!(
        layer.words.iter().map(String::as_str).collect::<Vec<_>>(),
        NEW_FORMS
    );
    assert_eq!(layer.next_words.len(), 4);
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
    let old = indexed_occurrences(&en.word_layers()[..54]);
    let all = indexed_occurrences(en.word_layers());
    assert_eq!(all.len(), 6213);
    assert_eq!(&all[..6187], old.as_slice());
    assert_eq!(
        all[6187..6193]
            .iter()
            .map(|(word, _)| word.as_str())
            .collect::<Vec<_>>(),
        NEW_FORMS
    );
    assert_eq!(
        all[6187..6193]
            .iter()
            .map(|(_, rank)| *rank)
            .collect::<Vec<_>>(),
        (19042..19048).collect::<Vec<_>>()
    );
    for word in NEW_FORMS {
        assert!(is_known_english_word(word), "{word}");
        let mut ime = engine(word);
        draft(&ime, word);
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        commit_once(&mut ime, word, CandidateKind::Word);
    }
    for (partial, word) in [
        ("shee", "sheet"),
        ("shee", "sheets"),
        ("pillo", "pillows"),
        ("bedsi", "bedside"),
        ("slipp", "slippers"),
        ("pajam", "pajamas"),
    ] {
        let ime = engine(partial);
        draft(&ime, partial);
        index(&ime, word, CandidateKind::Word);
    }
    // Bare bed/she/pa/sl are crowded roots, not promises of new first-page words.
}

#[test]
fn bedding_eight_specific_branches_keep_page_one_words_authors_and_exact_commits() {
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
fn bedding_preserves_case_smart_apostrophes_six_gaps_and_adopted_continuation() {
    for (seed, word, sentence) in [
        (
            "OUR SPARE BED STILL NEEDS THE SHEE",
            "OUR SPARE BED STILL NEEDS THE SHEET",
            "OUR SPARE BED STILL NEEDS THE SHEET from the clean laundry.",
        ),
        (
            "My slippers and pajamas are ready HER",
            "My slippers and pajamas are ready HERE",
            "My slippers and pajamas are ready HERE on the chair.",
        ),
        (
            "THE BEDSIDE LAMP IS ON THE TABLE NEXT TO THE BOOK I’M REA",
            "THE BEDSIDE LAMP IS ON THE TABLE NEXT TO THE BOOK I’M READING",
            "THE BEDSIDE LAMP IS ON THE TABLE NEXT TO THE BOOK I’M READING.",
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
    for (i, partial_tail, full_tail) in [
        (0, "fro", "from"),
        (1, "wi", "with"),
        (2, "i", "in"),
        (3, "nex", "next"),
    ] {
        let (context, next, partial, sentence) =
            (CONTEXTS[i], WORDS[i][0], PARTIALS[i][0], SENTENCES[i][0]);
        let word = format!("{context} {next}");
        let rest = sentence.strip_prefix(&format!("{word} ")).unwrap();
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
fn bedding_keeps_old_laundry_and_bedtime_controls_and_protected_line_and_unicode_boundaries() {
    for word in [
        "bed",
        "pillow",
        "blankets",
        "bedsheets",
        "pillowcases",
        "bedtime",
        "lamp",
        "laundry",
    ] {
        assert!(is_known_english_word(word), "{word}");
        raw_once(word);
    }
    for (seed, word, sentence) in [
        (
            "i'm getting ready for b",
            "i'm getting ready for bed",
            "i'm getting ready for bed.",
        ),
        (
            "could you help me fold the bla",
            "could you help me fold the blankets",
            "could you help me fold the blankets?",
        ),
        (
            "i'll put the folded laundry h",
            "i'll put the folded laundry here",
            "i'll put the folded laundry here on your bed.",
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
        "未完成🙂 pillows",
        "https://example.invalid/shee",
        "pillow_she",
        "xyzbedside",
    ] {
        raw_once(seed);
    }
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "bedding-synthetic-missing"
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
fn bedding_complete_local_pools_survive_pending_unavailable_timeout_and_empty_models() {
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

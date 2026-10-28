//! Authored call-recovery utterances, not media control or an actual network call.
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
    "your video is",
    "the sound is",
    "i can hear my own",
    "let me switch to my",
    "i'll leave the call and",
    "please wait while i",
];
const WORDS: [[&str; 2]; 6] = [
    ["frozen", "clear"],
    ["ahead", "behind"],
    ["voice", "words"],
    ["headphones", "phone"],
    ["connect", "join"],
    ["connect", "switch"],
];
const PARTIALS: [[&str; 2]; 6] = [
    ["froze", "clea"],
    ["ahea", "behin"],
    ["voic", "word"],
    ["headphone", "phon"],
    ["connec", "joi"],
    ["connec", "switc"],
];
const SENTENCES: [[&str; 2]; 6] = [
    [
        "your video is frozen but i can still hear you.",
        "your video is clear now that the camera is working again.",
    ],
    [
        "the sound is ahead of the video on my screen.",
        "the sound is behind the video so it's hard to follow.",
    ],
    [
        "i can hear my own voice coming back through the speakers.",
        "i can hear my own words a moment after i speak.",
    ],
    [
        "let me switch to my headphones so we can try the sound again.",
        "let me switch to my phone and join the call from there.",
    ],
    [
        "i'll leave the call and connect again in a minute.",
        "i'll leave the call and join again from my phone.",
    ],
    [
        "please wait while i connect my headphones.",
        "please wait while i switch to another device.",
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
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| { c.source == CandidateSource::Local && c.text.starts_with(seed) })
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
fn call_layer_has_fixed_records_without_new_or_reordered_index_forms() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 968);
    assert_eq!(en.sentences().count(), 1900);
    let previous = &en.word_layers()[..44];
    assert_eq!(
        previous.iter().map(|l| l.next_words.len()).sum::<usize>(),
        803
    );
    assert_eq!(
        previous.iter().map(|l| l.sentences.len()).sum::<usize>(),
        1570
    );
    let layer = &en.word_layers()[44];
    assert_eq!(layer.id, "daily_call");
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
            .all(|sentence| { previous.iter().all(|old| !old.sentences.contains(sentence)) })
    );
    let old = indexed_forms(previous);
    assert_eq!(old.len(), 6143);
    let current = indexed_forms(en.word_layers());
    assert_eq!(current.len(), 6216);
    assert_eq!(&current[..6143], old.as_slice());
}

#[test]
fn call_fixed_branches_offer_first_page_word_sentence_and_raw_once_only_commit() {
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
fn call_keeps_case_curly_apostrophes_and_six_horizontal_separators() {
    for (seed, word, sentence) in [
        (
            "I’ll leave the call and JOI",
            "I’ll leave the call and JOIN",
            "I’ll leave the call and JOIN again from my phone.",
        ),
        (
            "THE SOUND IS BEHIN",
            "THE SOUND IS BEHIND",
            "THE SOUND IS BEHIND the video so it's hard to follow.",
        ),
        (
            "Your video is FROZE",
            "Your video is FROZEN",
            "Your video is FROZEN but i can still hear you.",
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
fn call_word_adoption_space_tail_progress_and_reseeding_do_not_commit() {
    let tails = [
        (0, "bu", "but"),
        (0, "o", "of"),
        (0, "comin", "coming"),
        (1, "an", "and"),
        (1, "agai", "again"),
        (1, "t", "to"),
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
fn call_preserves_old_controls_and_rejects_unknown_protected_or_cross_line_phrases() {
    for (seed, word, sentence) in [
        (
            "can you hear me clearl",
            "can you hear me clearly",
            "can you hear me clearly?",
        ),
        (
            "your voice keeps breakin",
            "your voice keeps breaking",
            "your voice keeps breaking up.",
        ),
        (
            "i'll call you back from ho",
            "i'll call you back from home",
            "i'll call you back from home.",
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
        for prefix in ["https://('", "file_name_('", "unknownprefix"] {
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
        "call-synthetic-missing"
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
fn call_exact_local_pools_survive_unavailable_timeout_and_empty_model_results() {
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

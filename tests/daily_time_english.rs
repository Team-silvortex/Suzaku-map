//! Fixed authored time/waiting/rescheduling fallback, not live calendar access.
//! Expectations are frozen before data changes; the old index/ranks stay intact.
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{WordLayer, builtin},
};

const TIME: &[(&str, [(&str, &str); 2])] = &[
    (
        "can we move this to",
        [
            ("tomorrow", "can we move this to tomorrow morning?"),
            ("friday", "can we move this to friday afternoon?"),
        ],
    ),
    (
        "would it work better",
        [
            ("earlier", "would it work better earlier in the day?"),
            ("later", "would it work better later in the afternoon?"),
        ],
    ),
    (
        "what time works best",
        [
            ("tomorrow", "what time works best tomorrow?"),
            ("today", "what time works best today?"),
        ],
    ),
    (
        "could we meet a little",
        [
            ("earlier", "could we meet a little earlier tomorrow?"),
            ("later", "could we meet a little later this afternoon?"),
        ],
    ),
    (
        "how much longer should we",
        [
            ("wait", "how much longer should we wait here?"),
            ("stay", "how much longer should we stay here?"),
        ],
    ),
    (
        "i may be a few minutes",
        [
            ("late", "i may be a few minutes late."),
            ("early", "i may be a few minutes early."),
        ],
    ),
    (
        "please give me another",
        [
            ("minute", "please give me another minute to get ready."),
            ("moment", "please give me another moment to check the time."),
        ],
    ),
    (
        "let me know if the time",
        [
            ("changes", "let me know if the time changes."),
            ("works", "let me know if the time works for you."),
        ],
    ),
    (
        "can you remind me",
        [
            ("tomorrow", "can you remind me tomorrow morning?"),
            ("later", "can you remind me later today?"),
        ],
    ),
    (
        "i'll be there in a few",
        [
            ("minutes", "i'll be there in a few minutes, please wait."),
            ("more", "i'll be there in a few more minutes."),
        ],
    ),
    (
        "does tomorrow still work",
        [
            ("for", "does tomorrow still work for you?"),
            ("better", "does tomorrow still work better for you?"),
        ],
    ),
    (
        "let's leave a little",
        [
            (
                "earlier",
                "let's leave a little earlier so we have more time.",
            ),
            (
                "later",
                "let's leave a little later if there's enough time.",
            ),
        ],
    ),
];

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

fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn time_word_and_sentence_branches_are_on_page_one_for_full_and_partial_contexts() {
    assert_eq!(TIME.len(), 12);
    for (context, branches) in TIME {
        for (next, sentence) in branches {
            let word = format!("{context} {next}");
            for seed in [format!("{context} "), format!("{context} {}", &next[..2])] {
                let ime = engine(&seed);
                draft(&ime, &seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                index(&ime, &word, CandidateKind::Word);
                index(&ime, sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn every_time_branch_and_sentence_commits_once_and_undoes_once() {
    for (context, branches) in TIME {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..2]);
            commit_once(
                &mut engine(&seed),
                &format!("{context} {next}"),
                CandidateKind::Word,
            );
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            // Complete known tokens retain their preexisting Word label,
            // even when selecting the unchanged draft rather than a completion.
            let raw_kind = if matches!(&next[..2], "to" | "be") {
                CandidateKind::Word
            } else {
                CandidateKind::Literal
            };
            commit_once(&mut engine(&seed), &seed, raw_kind);
        }
    }
}

#[test]
fn time_preserves_horizontal_spaces_case_apostrophes_and_literal_text() {
    for (seed, word, sentence, raw_kind) in [
        (
            "Can we move this to TO",
            "Can we move this to TOMORROW",
            "Can we move this to TOMORROW morning?",
            CandidateKind::Word,
        ),
        (
            "  please  give  me  another  mi",
            "  please  give  me  another  minute",
            "  please  give  me  another  minute to get ready.",
            CandidateKind::Literal,
        ),
        (
            "I’ll be there in a few mi",
            "I’ll be there in a few minutes",
            "I’ll be there in a few minutes, please wait.",
            CandidateKind::Literal,
        ),
        (
            "let’s leave a little ea",
            "let’s leave a little earlier",
            "let’s leave a little earlier so we have more time.",
            CandidateKind::Literal,
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        index(&ime, sentence, CandidateKind::Sentence);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_once(&mut engine(seed), seed, raw_kind);
    }
}

#[test]
fn long_time_drafts_and_adopted_words_keep_exact_prefixes_and_spacing() {
    for prefix in ["Earlier note. ".repeat(40), "前文😀。  ".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}please give me another mi");
            let word = format!("{prefix}{separator}please give me another minute");
            let sentence = format!("{word} to get ready.");
            let mut ime = engine(&seed);
            assert!(seed.chars().count() > 256);
            draft(&ime, &seed);
            commit_once(&mut ime, &sentence, CandidateKind::Sentence);
            ime.seed(&seed);
            ime.select_candidate(index(&ime, &word, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
            // Host-like adoption and reseeding, not desktop physical-event acceptance.
            for tail in ["  ", "  to", "  to "] {
                let continued = format!("{word}{tail}");
                ime.seed(&continued);
                draft(&ime, &continued);
                index(
                    &ime,
                    &format!("{word}  to get ready."),
                    CandidateKind::Sentence,
                );
            }
            commit_once(
                &mut ime,
                &format!("{word}  to get ready."),
                CandidateKind::Sentence,
            );
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn time_does_not_manufacture_authored_sentences_across_lines_or_protected_text() {
    for separator in ["\n", "\r", "\u{85}", "\u{2028}", "\u{2029}"] {
        let seed = format!("please give me{separator}another mi");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text
            == format!("please give me{separator}another minute to get ready.")
            && c.kind == CandidateKind::Sentence));
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
    for (seed, forbidden) in [
        (
            "https://('please give me another mi",
            "https://('please give me another minute to get ready.",
        ),
        (
            "file_name_('please give me another mi",
            "file_name_('please give me another minute to get ready.",
        ),
        (
            "unknownprefixplease give me another mi",
            "unknownprefixplease give me another minute to get ready.",
        ),
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.text == forbidden && c.kind == CandidateKind::Sentence)
        );
        commit_once(&mut ime, seed, CandidateKind::Literal);
    }
    // An ordinary wrapper must still permit the same phrase completion.
    commit_once(
        &mut engine("('please give me another mi"),
        "('please give me another minute to get ready.",
        CandidateKind::Sentence,
    );
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "time-synthetic-unavailable"
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
fn time_local_fallback_survives_missing_failed_timed_out_and_empty_providers() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in TIME.iter().step_by(3) {
            let (next, sentence) = branches[0];
            let seed = format!("{context} {}", &next[..2]);
            let mut ime = engine(&seed);
            ime.configure_prediction(Some(Arc::new(MissingProvider(error.clone()))));
            ime.seed(&seed);
            draft(&ime, &seed);
            index(&ime, &format!("{context} {next}"), CandidateKind::Word);
            index(&ime, sentence, CandidateKind::Sentence);
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
            commit_once(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

fn hash_pairs<'a>(pairs: impl Iterator<Item = (&'a str, &'a [String])>) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for (context, values) in pairs {
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        hash = (hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    hash
}

fn word_positions(layers: &[WordLayer]) -> BTreeMap<String, usize> {
    let words = layers.iter().flat_map(|l| {
        l.words
            .iter()
            .map(String::as_str)
            .chain(
                l.next_words
                    .iter()
                    .flat_map(|(_, words)| words.iter().map(String::as_str)),
            )
            .chain(l.sentences.iter().flat_map(|s| {
                s.split(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '\'' | '’')))
                    .filter(|w| !w.is_empty())
            }))
    });
    let mut positions = BTreeMap::new();
    for (rank, word) in words.enumerate() {
        positions
            .entry(word.replace('’', "'").to_ascii_lowercase())
            .or_insert(rank);
    }
    positions
}

#[test]
fn time_preserves_every_old_layer_context_sentence_and_index_rank() {
    let en = builtin("en").unwrap();
    let previous = &en.word_layers()[..22];
    assert_eq!(
        hash_pairs(previous.iter().map(|l| (l.id.as_str(), l.words.as_slice()))),
        0xc0485a4cc9a2414b
    );
    assert_eq!(hash_pairs(en.next_words().take(609)), 0xba425b4f9b7a6549);
    assert_eq!(
        hash_pairs(en.sentences().take(1182).map(|t| (t, &[][..]))),
        0x3edb8eb03f6fe11c
    );
    let before = word_positions(previous);
    let current = word_positions(en.word_layers());
    assert_eq!(before.len(), 6139);
    for (word, rank) in before {
        assert_eq!(current[&word], rank, "changed old rank for {word}");
    }
    assert_eq!(current.len(), 6216);
}

#[test]
fn time_adds_twelve_fixed_contexts_twenty_four_sentences_without_index_growth() {
    let en = builtin("en").unwrap();
    let layer = en
        .word_layers()
        .get(22)
        .expect("missing authored time layer");
    assert_eq!(layer.id, "daily_time");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 12);
    assert_eq!(layer.sentences.len(), 24);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(TIME) {
        assert_eq!(context, expected);
        assert_eq!(
            words.iter().map(String::as_str).collect::<Vec<_>>(),
            branches.map(|(word, _)| word)
        );
    }
    assert_eq!(
        layer
            .sentences
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        TIME.iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, s)| *s))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        word_positions(&en.word_layers()[..22]),
        word_positions(&en.word_layers()[..23])
    );
}

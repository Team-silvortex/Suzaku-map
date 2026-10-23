//! Two authored readings, not an unrestricted dictionary or semantic generator.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{self, EntryKind},
};

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()))
}

fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
    index(ime, seed, CandidateKind::Literal);
}

fn primary(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert_eq!(index(ime, text, kind), 0);
    assert_eq!(ime.candidates()[0].score, 100.0);
}

fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

fn signature(ime: &XRTabletImeEngine) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for c in ime.candidates() {
        for byte in c
            .text
            .bytes()
            .chain([0])
            .chain(c.label.bytes())
            .chain([0])
            .chain(c.kind.id().bytes())
            .chain([0])
            .chain(c.source.id().bytes())
            .chain([0])
            .chain(c.score.to_bits().to_le_bytes())
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    hash
}

#[test]
fn exactly_two_words_append_after_the_previous_character_window_without_authored_sentences() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(zh.readings().len(), 3410);
    assert_eq!(zh.continuations().count(), 861);
    assert_eq!(
        zh.continuations().map(|(_, v)| v.len()).sum::<usize>(),
        1720
    );
    for (entry, (reading, word)) in zh.readings()[3162..3166].iter().zip([
        ("jie", "借"),
        ("huan", "还"),
        ("gei", "给"),
        ("shu", "书"),
    ]) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    assert!(
        !zh.readings()
            .iter()
            .any(|e| e.reading == "ge" && e.text == "个")
    );
}

#[test]
fn short_prefixes_and_prior_characters_keep_real_before_data_full_vectors() {
    // Captured from before-probe.log, including positions, labels and score bits.
    for (seed, expected) in [
        ("g", 0x24c152833426a4ff),
        ("ge", 0xa67c37e667dfbfec),
        ("sh", 0xc713aaae0666bd39),
        ("hai", 0x43081278a2a4c99c),
        ("jie", 0xa7d6f6af2c16488d),
        ("huan", 0xc8c94b70db3cfef1),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(signature(&ime), expected, "{seed}: {:?}", ime.candidates());
    }
}

#[test]
fn exact_words_case_tone_and_horizontal_padding_keep_raw_and_once_only_commit() {
    for (readings, word) in [
        (["gei", "GEI", "gei3", "GEI3"], "给"),
        (["shu", "SHU", "shu1", "SHU1"], "书"),
    ] {
        for reading in readings {
            for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
                let seed = format!("{reading}{gap}");
                let ime = engine(&seed);
                draft(&ime, &seed);
                primary(&ime, word, CandidateKind::Word);
                commit_once(&mut engine(&seed), word, CandidateKind::Word);
                commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn new_exact_words_precede_old_broad_completions_without_removing_them() {
    for (seed, word, old, extra) in [
        (
            "gei",
            "给",
            ["给花浇水", "给我留一个", "给自己放个假", "给我点提示"],
            None,
        ),
        ("shu", "书", ["输入法", "输入", "说", "水果"], Some("蔬菜")),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        primary(&ime, word, CandidateKind::Word);
        let positions = old.map(|text| index(&ime, text, CandidateKind::Word));
        assert!(positions.windows(2).all(|p| p[0] < p[1]));
        for text in old.into_iter().chain(extra) {
            commit_once(&mut engine(seed), text, CandidateKind::Word);
        }
    }
}

#[test]
fn established_compounds_homophones_and_unfinished_ge_keep_primary_choices() {
    for (reading, word) in [
        ("shu'ru'fa", "输入法"),
        ("shu'ru", "输入"),
        ("shu'ju", "数据"),
        ("shu'ji", "书籍"),
        ("jie'shu", "结束"),
        ("huan'shu", "还书"),
        ("gei'hua'jiao'shui", "给花浇水"),
        ("gei'wo'liu'yi'ge", "给我留一个"),
        ("gei'wo'dian'ti'shi", "给我点提示"),
    ] {
        for seed in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            primary(&ime, word, CandidateKind::Word);
            if word == "结束" {
                assert!(index(&ime, "借书", CandidateKind::Word) < PAGE_SIZE);
                assert!(
                    index(&ime, "结束", CandidateKind::Word)
                        < index(&ime, "借书", CandidateKind::Word)
                );
            }
        }
    }
    for seed in ["huange", "huan ge"] {
        primary(&engine(seed), "换个说法", CandidateKind::Word);
    }
    primary(&engine("jie4shu1"), "结束", CandidateKind::Word);
}

#[test]
fn giving_and_book_spans_compose_without_new_fixed_phrase_entries() {
    for (reading, sentence) in [
        ("gei'ni", "给你"),
        ("gei'wo", "给我"),
        ("wo'gei'ni", "我给你"),
        ("gei'wo'shu", "给我书"),
        ("wo'gei'ni'shu", "我给你书"),
    ] {
        for seed in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(index(&ime, sentence, CandidateKind::Sentence) < PAGE_SIZE);
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn adopted_characters_and_mixed_han_preserve_editing_and_literal_spacing() {
    for (reading, word, tail, expected, kind) in [
        ("gei", "给", "ni", "给你", CandidateKind::Sentence),
        ("shu", "书", "gei ni", "书给你", CandidateKind::Sentence),
        ("jie", "借", "shu", "借书", CandidateKind::Word),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        let seed = format!("{word}{tail}");
        ime.seed(&seed);
        draft(&ime, &seed);
        index(&ime, expected, kind);
        commit_once(&mut ime, expected, kind);
        for gap in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
            let seed = format!("{word}{gap}{tail}");
            let spaced = format!("{word}{gap}{}", &expected[word.len()..]);
            let ime = engine(&seed);
            draft(&ime, &seed);
            index(&ime, &spaced, CandidateKind::Sentence);
            commit_once(&mut engine(&seed), &spaced, CandidateKind::Sentence);
        }
    }
    for (seed, text) in [("借gei ni", "借给你"), ("还gei wo", "还给我")] {
        draft(&engine(seed), seed);
        commit_once(&mut engine(seed), text, CandidateKind::Sentence);
    }
    let ime = engine("用完还gei ni");
    draft(&ime, "用完还gei ni");
    index(&ime, "用完还给你", CandidateKind::Word);
    index(&ime, "用完还给你，不会拿走的。", CandidateKind::Sentence);
    for prefix in ["已确认 API。".to_owned(), "前文 café 😀。  ".repeat(80)] {
        for (reading, word) in [("gei", "给"), ("shu", "书")] {
            let seed = format!("{prefix}{reading}");
            let converted = format!("{prefix}{word}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
            commit_once(&mut engine(&seed), &converted, CandidateKind::Word);
        }
    }
}

#[test]
fn punctuation_numbers_wrong_splits_and_line_or_token_boundaries_are_not_erased() {
    for (seed, text) in [
        ("gei'", "给'"),
        ("shu'", "书'"),
        ("gei3.5", "给3.5"),
        ("shu1:30", "书1:30"),
        ("gei40", "给40"),
    ] {
        draft(&engine(seed), seed);
        commit_once(&mut engine(seed), text, CandidateKind::Sentence);
        assert!(
            !engine(seed)
                .candidates()
                .iter()
                .any(|c| c.text == "给" || c.text == "书")
        );
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for seed in [
        "g ei",
        "ge i",
        "s hu",
        "sh u",
        "https://example.invalid/gei",
        "person+shu@example.invalid",
        "file_gei",
        "书zzqvjy",
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.text == "给" || c.text == "书")
        );
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("gei{boundary}wo");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == "给我"));
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
    for seed in ["给", "书"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.kind == CandidateKind::Word && c.text == seed)
        );
    }
}

struct FailedProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedProvider {
    fn provider_id(&self) -> &str {
        "giving-characters-synthetic-error"
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
fn unavailable_timeout_and_empty_model_keep_all_immediate_local_choices() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (reading, word) in [("gei", "给"), ("shu", "书")] {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedProvider(error.clone()))));
            ime.seed(reading);
            draft(&ime, reading);
            primary(&ime, word, CandidateKind::Word);
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
            commit_once(&mut ime, word, CandidateKind::Word);
        }
    }
}

//! A known Word's aligned complete syllable can verify an adopted author's Han prefix.
//! No arbitrary homophone correction, extra decoder paths, native keys or real models.
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
    lexicon::{self, EntryKind},
};

const CONTEXT: &str = "门垫旁边先留空";
const AUTHOR: &str = "门垫旁边先留空，袋子我来挪开。";

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    assert!(ime.candidates().iter().all(|c| {
        c.source == CandidateSource::Local
            && c.label == c.text
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
    assert!(ime.candidates().iter().any(|c| {
        c.text == seed && c.kind == CandidateKind::Literal && c.source == CandidateSource::Local
    }));
}
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}
fn commit_once(seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(seed);
    draft(&ime, seed);
    ime.select_candidate(index(&ime, text, kind));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.selected_completion_text(true), Some(text));
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

#[test]
fn adopted_complete_syllable_keeps_real_old_anchor_words_and_restores_its_author() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert!(zh.readings().iter().any(|entry| {
        entry.reading == "dai'kou'xian'ji'hao"
            && entry.text == "袋口先系好"
            && entry.kind == EntryKind::Word
            && !entry.require_separators
    }));
    // This is prefix evidence from the existing five-syllable Word, not an
    // unreported new 袋/袋子 dictionary entry or a rewritten homophone order.
    assert!(
        !zh.readings()
            .iter()
            .any(|entry| matches!(&*entry.text, "袋" | "袋子"))
    );
    let seed = "门垫旁边先留空，DAI";
    let ime = engine(seed);
    draft(&ime, seed);
    assert_eq!(ime.candidates()[0].text, "门垫旁边先留空，带");
    assert_eq!(ime.candidates()[0].kind, CandidateKind::Sentence);
    assert_eq!(ime.candidates()[0].score, 100.0);
    let positions = [
        ("门垫旁边先留空，代码", 92.0_f32),
        ("门垫旁边先留空，待办", 90.0),
        ("门垫旁边先留空，待办事项", 88.0),
        ("门垫旁边先留空，代收", 86.0),
    ]
    .map(|(text, score)| {
        let at = index(&ime, text, CandidateKind::Word);
        assert_eq!(ime.candidates()[at].score.to_bits(), score.to_bits());
        at
    });
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    index(&ime, AUTHOR, CandidateKind::Sentence);
    commit_once(seed, AUTHOR, CandidateKind::Sentence);
    commit_once(seed, seed, CandidateKind::Literal);
}

#[test]
fn complete_syllable_case_tones_horizontal_padding_and_long_prefixes_keep_exact_written_text() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for gap in ["", " ", "\t", "\u{a0}", "\u{202f}", "\u{3000}"] {
            for reading in ["dai", "DAI", "dai4", "DAI4"] {
                let written = format!("{prefix}{CONTEXT}{gap}，");
                let raw = format!("{written}{reading}");
                let expected = format!("{written}袋子我来挪开。");
                let ime = engine(&raw);
                draft(&ime, &raw);
                assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
                index(&ime, &expected, CandidateKind::Sentence);
                commit_once(&raw, &expected, CandidateKind::Sentence);
                commit_once(&raw, &raw, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn incomplete_syllables_wrong_han_tokens_quotes_numbers_and_lines_do_not_borrow_the_author() {
    for tail in [
        "da", "daiq", "d a i", "da i", "dai'", "dai4.5", "dai2:30", "zzqvjy",
    ] {
        let raw = format!("{CONTEXT}，{tail}");
        let ime = engine(&raw);
        draft(&ime, &raw);
        assert!(!ime.candidates().iter().any(|c| c.text == AUTHOR));
        commit_once(&raw, &raw, CandidateKind::Literal);
    }
    for raw in [
        "门垫旁边先留控，DAI",
        "门垫旁边先留空，带DAI",
        "https://example.invalid/门垫旁边先留空，DAI",
        "file_name_门垫旁边先留空，DAI",
        "mail@example.invalid/门垫旁边先留空，DAI",
    ] {
        let ime = engine(raw);
        draft(&ime, raw);
        assert!(!ime.candidates().iter().any(|c| c.text.contains(AUTHOR)));
        commit_once(raw, raw, CandidateKind::Literal);
    }
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let raw = format!("{CONTEXT}{line}，DAI");
        let ime = engine(&raw);
        draft(&ime, &raw);
        assert!(!ime.candidates().iter().any(|c| {
            c.text == AUTHOR || c.text == format!("{CONTEXT}{line}，袋子我来挪开。")
        }));
        commit_once(&raw, &raw, CandidateKind::Literal);
    }
}

struct MissingModel(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingModel {
    fn provider_id(&self) -> &str {
        "adopted-syllable-prefix-synthetic"
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
fn pending_failed_timeout_and_empty_models_preserve_the_complete_verified_local_pool() {
    let seed = "门垫旁边先留空，DAI";
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::InvalidResponse),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        let mut ime = engine(seed);
        let local = ime.candidates().to_vec();
        index(&ime, AUTHOR, CandidateKind::Sentence);
        ime.configure_prediction(Some(Arc::new(MissingModel(error.clone()))));
        ime.seed(seed);
        draft(&ime, seed);
        assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
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
        draft(&ime, seed);
        assert_eq!(ime.candidates(), local);
        commit_once(seed, AUTHOR, CandidateKind::Sentence);
    }
}

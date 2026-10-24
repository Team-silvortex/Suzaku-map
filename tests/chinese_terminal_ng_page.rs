//! A verified whole Word ending in ng can retain its own two authored rows.
//! This exercises the actual decoder/engine, not private routing helpers or models.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, LanguagePlugin, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE, offline},
    },
    languages::{
        chinese::ChineseLanguagePlugin,
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
};

const SEED: &str = "xie'gui'li'hai'you'yi'shuan";
const COMPLETE: &str = "xie'gui'li'hai'you'yi'shuang";
const WORD: &str = "鞋柜里还有一双";
const ANCHOR: &str = "鞋柜厉害有以书安";
const AUTHORS: [&str; 2] = [
    "鞋柜里还有一双，我帮你拿出来。",
    "鞋柜里还有一双，放在下面那层。",
];

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
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 2 * PAGE_SIZE);
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
    index(ime, seed, CandidateKind::Literal);
}
fn pair(ime: &XRTabletImeEngine, prefix: &str) {
    for author in AUTHORS {
        let expected = format!("{prefix}{author}");
        assert!(
            index(ime, &expected, CandidateKind::Sentence) < PAGE_SIZE,
            "own pair left first page: {expected:?}: {:?}",
            ime.candidates()
        );
    }
}
fn commit_once(seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(seed);
    draft(&ime, seed);
    ime.select_candidate(index(&ime, text, kind));
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
fn signature(ime: &XRTabletImeEngine) -> u64 {
    let mut h = 0xcbf29ce484222325_u64;
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
            h = (h ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    h
}

#[test]
fn terminal_ng_keeps_real_anchor_word_order_scores_and_own_pair_on_default_first_page() {
    // These complete tuples come from the retained actual compiled-before probe.
    // No decoder choice, ranking weight or source is rewritten by this page repair.
    let ime = engine(SEED);
    draft(&ime, SEED);
    assert_eq!(ime.candidates()[0].text, ANCHOR);
    assert_eq!(ime.candidates()[0].kind, CandidateKind::Sentence);
    assert_eq!(ime.candidates()[0].score.to_bits(), 100.0_f32.to_bits());
    let positions = [
        (WORD, 90.0_f32),
        ("鞋柜厉害有以双击", 88.0),
        ("鞋柜厉害有以双重验证", 86.0),
        ("鞋柜厉害有以双面打印", 84.0),
    ]
    .map(|(text, score)| {
        let at = index(&ime, text, CandidateKind::Word);
        assert_eq!(ime.candidates()[at].score.to_bits(), score.to_bits());
        at
    });
    assert!(positions.windows(2).all(|p| p[0] < p[1]));
    for (author, score) in AUTHORS.into_iter().zip([78.5_f32, 78.0]) {
        assert_eq!(
            ime.candidates()[index(&ime, author, CandidateKind::Sentence)]
                .score
                .to_bits(),
            score.to_bits()
        );
    }
    assert_eq!(
        ime.candidates()[index(&ime, SEED, CandidateKind::Literal)].score,
        55.0
    );
    pair(&ime, "");
}

#[test]
fn terminal_ng_case_legal_reading_gaps_and_frozen_long_prefixes_preserve_raw_and_pair() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for gap in ["'", " ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
            let reading = SEED.replace('\'', gap);
            for spelling in [reading.clone(), reading.to_ascii_uppercase()] {
                let raw = format!("{prefix}{spelling}");
                let ime = engine(&raw);
                draft(&ime, &raw);
                assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
                pair(&ime, &prefix);
                commit_once(&raw, &format!("{prefix}{WORD}"), CandidateKind::Word);
                for author in AUTHORS {
                    commit_once(&raw, &format!("{prefix}{author}"), CandidateKind::Sentence);
                }
                commit_once(&raw, &raw, CandidateKind::Literal);
            }
        }
        for reading in [
            "xieguilihaiyouyishuan",
            "xie2'gui4'li3'hai2'you3'yi4'shuan",
            "XIE2 GUI4 LI3 HAI2 YOU3 YI4 SHUAN",
        ] {
            let raw = format!("{prefix}{reading}");
            let ime = engine(&raw);
            draft(&ime, &raw);
            pair(&ime, &prefix);
            commit_once(&raw, &raw, CandidateKind::Literal);
        }
    }
}

#[test]
fn terminal_ng_full_reading_adoption_explicit_submissions_and_undo_remain_independent() {
    for seed in [
        COMPLETE.to_owned(),
        COMPLETE.replace('\'', " "),
        COMPLETE.replace('\'', ""),
    ] {
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert_eq!(ime.candidates()[0].text, WORD);
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        pair(&ime, "");
        commit_once(&seed, WORD, CandidateKind::Word);
        for author in AUTHORS {
            commit_once(&seed, author, CandidateKind::Sentence);
        }
        commit_once(&seed, &seed, CandidateKind::Literal);
    }
    let mut ime = engine(SEED);
    ime.select_candidate(index(&ime, WORD, CandidateKind::Word));
    assert_eq!(ime.selected_completion_text(true), Some(WORD));
    assert_eq!(ime.snapshot().seed_text, SEED);
    assert!(ime.snapshot().committed_text.is_empty());
    ime.seed(WORD);
    draft(&ime, WORD);
    pair(&ime, "");
    for author in AUTHORS {
        commit_once(WORD, author, CandidateKind::Sentence);
    }
    commit_once(WORD, WORD, CandidateKind::Literal);
}

#[test]
fn terminal_ng_rejects_unrelated_ambiguity_malformed_boundaries_and_token_recovery() {
    // The old multichar ambiguity is deliberately not widened into this guard.
    let old = engine("huan ge");
    draft(&old, "huan ge");
    assert_eq!(signature(&old), 0x999081dd912ed081);
    for raw in [
        format!("{SEED}'"),
        format!("{SEED}6"),
        format!("{SEED} g"),
        "xie'gui'li'hai'you'yi'sh'uan".into(),
        "xie'gui'li'hai'you'yi'shu'an".into(),
        "xie'gui'li'hai'you'yi'shuangx".into(),
        format!("https://example.invalid/{SEED}"),
        format!("user+{SEED}@example.invalid"),
        format!("file_{SEED}"),
        format!("unknown{SEED}"),
    ] {
        let ime = engine(&raw);
        draft(&ime, &raw);
        assert!(
            ime.candidates()
                .iter()
                .all(|c| !AUTHORS.iter().any(|a| c.text.contains(a))),
            "{raw:?}: {:?}",
            ime.candidates()
        );
        commit_once(&raw, &raw, CandidateKind::Literal);
    }
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let raw = format!("xie'gui'li'hai'you'yi{boundary}shuan");
        let ime = engine(&raw);
        draft(&ime, &raw);
        assert!(
            ime.candidates()
                .iter()
                .all(|c| !AUTHORS.iter().any(|a| c.text.contains(a)))
        );
        commit_once(&raw, &raw, CandidateKind::Literal);
    }
}

#[test]
fn terminal_ng_small_public_mixer_limits_keep_old_anchor_literal_and_membership_contracts() {
    // The public IBus engine fixes its normal mixed capacity at twelve. Explicit
    // diagnostic smaller limits exercise the same public mixer, not a private helper.
    let full = engine(SEED).candidates().to_vec();
    for limit in 0..PAGE_SIZE {
        let choices = offline(
            "zh-Hans",
            SEED,
            "",
            ChineseLanguagePlugin.direct_candidates(SEED, 1.0).unwrap(),
            limit,
        );
        assert!(choices.len() <= limit.max(1));
        assert_eq!(choices[0], full[0]);
        assert!(choices.iter().all(|c| full.contains(c)));
        if limit > 1 {
            assert!(
                choices
                    .iter()
                    .any(|c| c.text == SEED && c.kind == CandidateKind::Literal)
            );
        }
        // No new two-author guarantee for tiny limits or literal rescue.
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "terminal-ng-page-synthetic-error"
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
fn terminal_ng_pending_errors_empty_and_real_ram_preferences_keep_bounded_local_choices() {
    let baseline = engine(SEED).candidates().to_vec();
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        let mut ime = engine("");
        ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
        ime.seed(SEED);
        assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
        assert_eq!(ime.candidates(), baseline);
        pair(&ime, "");
        let deadline = Instant::now() + Duration::from_secs(2);
        while ime.prediction_pending() && Instant::now() < deadline {
            ime.poll_prediction();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!ime.prediction_pending());
        assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
        assert_eq!(
            ime.prediction_error(),
            Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
        );
        assert_eq!(ime.candidates(), baseline);
        pair(&ime, "");
    }
    let mut learned = engine(SEED);
    learned.select_candidate(index(&learned, AUTHORS[1], CandidateKind::Sentence));
    assert!(learned.commit(CommitOptions { force: true }).ok);
    assert_eq!(learned.preference_count(), 1);
    learned.clear_session_context();
    learned.seed(SEED);
    draft(&learned, SEED);
    assert_eq!(learned.candidates()[0], baseline[0]);
    assert!(
        learned.candidates()[index(&learned, AUTHORS[1], CandidateKind::Sentence)].score > 78.0
    );
    pair(&learned, "");
    learned.clear_preferences();
    learned.seed(SEED);
    assert_eq!(learned.preference_count(), 0);
    assert_eq!(learned.candidates(), baseline);
}

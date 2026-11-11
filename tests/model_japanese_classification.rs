//! Frozen visible adapter replies and synthetic classification cases; no model or network.
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        Candidate, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, classify, merge_predictions},
    },
    prediction::{
        PredictionCancellation, PredictionCandidate, PredictionError, PredictionKind,
        PredictionProvider, PredictionRequest, PredictionResponse,
    },
};

const SEED: &str = "nihongo";
const SENTENCE: &str = "日本語で説明してください。";
// The two authorized live cases returned these adapter rows, with kind=null.
// Replay only accepted visible text; no raw API body, reasoning or credentials.
const CAPTURED_ROWS: [&str; 2] = [
    r#"[{"text":"日本語力","score_bias":0.55,"kind":null},{"text":"日本語で説明してください。","score_bias":0.5,"kind":null}]"#,
    r#"[{"text":"日本語訳","score_bias":0.55,"kind":null},{"text":"日本語で説明してください。","score_bias":0.5,"kind":null}]"#,
];

struct Frozen(Vec<PredictionCandidate>);

impl PredictionProvider for Frozen {
    fn predict(
        &self,
        request: &PredictionRequest,
        cancellation: &PredictionCancellation,
    ) -> Result<PredictionResponse, PredictionError> {
        cancellation.check()?;
        assert_eq!(request.input.language_id, "ja");
        assert_eq!(request.input.seed_text, SEED);
        assert!(request.input.context_before_cursor.is_empty());
        Ok(PredictionResponse::new(request, self.0.clone()))
    }
}

fn engine() -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    engine.set_preferences_enabled(false);
    engine.enable_ibus_candidate_mix();
    engine.seed(SEED);
    engine
}

fn run_worker(rows: Vec<PredictionCandidate>) -> (Vec<Candidate>, XRTabletImeEngine) {
    let mut engine = engine();
    let before = engine.candidates().to_vec();
    engine.configure_prediction_provider(Some(Arc::new(Frozen(rows))));
    let deadline = Instant::now() + Duration::from_secs(2);
    while engine.prediction_pending() && Instant::now() < deadline {
        engine.poll_prediction();
        if engine.prediction_pending() {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
    assert!(engine.prediction_error().is_none());
    assert_eq!(engine.snapshot().seed_text, SEED);
    assert!(engine.snapshot().committed_text.is_empty());
    (before, engine)
}

fn row(text: &str, kind: Option<PredictionKind>, bias: f32) -> PredictionCandidate {
    PredictionCandidate {
        text: text.into(),
        kind,
        score_bias: bias,
    }
}

fn merged(language: &str, rows: Vec<PredictionCandidate>) -> Vec<Candidate> {
    let local = if language == "ja" {
        engine().candidates().to_vec()
    } else {
        Vec::new()
    };
    let (merged, accepted) = merge_predictions(language, SEED, local, rows, 12);
    assert!(accepted);
    merged
}

fn find<'a>(rows: &'a [Candidate], text: &str) -> &'a Candidate {
    rows.iter()
        .find(|row| row.text == text)
        .unwrap_or_else(|| panic!("missing {text:?}: {rows:?}"))
}

fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

#[test]
fn frozen_untyped_live_responses_reach_distinct_word_and_sentence_slots_through_the_worker() {
    for (json, word) in CAPTURED_ROWS.into_iter().zip(["日本語力", "日本語訳"]) {
        let replies: Vec<PredictionCandidate> = serde_json::from_str(json).unwrap();
        assert!(replies.iter().all(|reply| reply.kind.is_none()));
        let (before, engine) = run_worker(replies);
        let after = engine.candidates();
        let first = &after[0];
        assert_eq!(first.text, before[0].text);
        assert_eq!(first.kind, before[0].kind);
        assert_eq!(first.source, CandidateSource::Local);
        let inferred_word = find(after, word);
        assert_eq!(inferred_word.kind, CandidateKind::Word);
        assert_eq!(inferred_word.source, CandidateSource::Model);
        close(inferred_word.score, 90.0 + 0.55 * 6.0);
        let sentence = find(after, SENTENCE);
        assert_eq!(sentence.kind, CandidateKind::Sentence);
        assert_eq!(sentence.source, CandidateSource::Model);
        close(sentence.score, 82.0 + 0.5 * 6.0);
        let retained_local: Vec<_> = after
            .iter()
            .filter(|row| row.source == CandidateSource::Local)
            .map(|row| &row.text)
            .collect();
        assert_eq!(
            retained_local,
            before.iter().map(|row| &row.text).collect::<Vec<_>>()
        );
        assert!(after.len() <= 12);
        assert!(
            after
                .iter()
                .all(|row| row.score.is_finite() && (0.0..=100.0).contains(&row.score))
        );
    }
}

#[test]
fn untyped_nominal_extensions_generalize_beyond_the_two_observed_words() {
    for word in [
        "日本語版",
        "通信量",
        "情報誌",
        "予約料",
        "映画版",
        "音楽集",
        "入力率",
        "会議費",
    ] {
        let rows = merged("ja", vec![row(word, None, 0.4)]);
        let candidate = find(&rows, word);
        assert_eq!(candidate.kind, CandidateKind::Word, "{word}");
        assert_eq!(candidate.source, CandidateSource::Model);
        close(candidate.score, 90.0 + 0.4 * 6.0);
    }
}

#[test]
fn context_clauses_particles_punctuation_spacing_and_unknown_stems_do_not_become_words() {
    for text in [
        "明日出発",
        "今日帰宅",
        "今夜出発",
        "明日日本語力",
        "日本語の力",
        "日本語 力",
        "日本語力。",
        "日本語を話す",
        "勉強したい",
        "べんきょうしたい",
        "未知複合力",
        "日本語力版",
        "日本語日本語日本語力",
        "通信量増加",
        "😀日本語力",
        "JLPT日本語力",
    ] {
        let rows = merged("ja", vec![row(text, None, 0.4)]);
        let candidate = find(&rows, text);
        assert_ne!(candidate.kind, CandidateKind::Word, "{text}");
        assert_eq!(candidate.source, CandidateSource::Model);
    }
    // Whole-string dictionary membership/offline classification is not broadened.
    for text in ["日本語力", "日本語訳", "日本語版", "通信量"] {
        assert_eq!(classify("ja", SEED, text), CandidateKind::Sentence);
    }
}

#[test]
fn explicit_hints_keep_authority_and_other_languages_do_not_get_japanese_suffix_rules() {
    for (text, hint, expected) in [
        (
            "日本語力",
            PredictionKind::Sentence,
            CandidateKind::Sentence,
        ),
        ("日本語訳", PredictionKind::Word, CandidateKind::Word),
        ("明日出発", PredictionKind::Word, CandidateKind::Word),
    ] {
        let rows = merged("ja", vec![row(text, Some(hint), 0.4)]);
        assert_eq!(find(&rows, text).kind, expected);
    }
    for language in ["en", "zh-Hans"] {
        let rows = merged(language, vec![row("日本語力", None, 0.4)]);
        assert_eq!(
            find(&rows, "日本語力").kind,
            CandidateKind::Sentence,
            "{language}"
        );
    }
    for alias in ["ja", "ja-JP"] {
        let rows = merged(alias, vec![row("日本語力", None, 0.4)]);
        assert_eq!(find(&rows, "日本語力").kind, CandidateKind::Word, "{alias}");
    }
}

#[test]
fn local_corroboration_preserves_its_original_kind_source_label_and_old_weight_rule() {
    let local = vec![
        Candidate {
            label: "local anchor".into(),
            text: "日本語".into(),
            score: 100.0,
            kind: CandidateKind::Word,
            source: CandidateSource::Local,
        },
        Candidate {
            label: "authored local sentence".into(),
            text: "日本語力".into(),
            score: 70.0,
            kind: CandidateKind::Sentence,
            source: CandidateSource::Local,
        },
        Candidate {
            label: SEED.into(),
            text: SEED.into(),
            score: 55.0,
            kind: CandidateKind::Literal,
            source: CandidateSource::Local,
        },
    ];
    let (rows, accepted) =
        merge_predictions("ja", SEED, local, vec![row("日本語力", None, 0.55)], 12);
    assert!(accepted);
    let corroborated = find(&rows, "日本語力");
    assert_eq!(corroborated.kind, CandidateKind::Sentence);
    assert_eq!(corroborated.source, CandidateSource::Local);
    assert_eq!(corroborated.label, "authored local sentence");
    close(corroborated.score, 82.0 + 0.55 * 6.0);
    assert_eq!(rows.iter().filter(|row| row.text == "日本語力").count(), 1);
}

#[test]
fn mixed_model_inference_keeps_the_twelve_row_limit_stable_family_order_and_bounded_weights() {
    let local: Vec<_> = [
        "日本語",
        "候補甲",
        "候補乙",
        "候補丙",
        "候補丁",
        "候補戊",
        "候補己",
        "候補庚",
        "候補辛",
        "候補壬",
        "候補癸",
        SEED,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, text)| Candidate {
        label: text.into(),
        text: text.into(),
        score: 100.0 - index as f32 * 2.0,
        kind: if text == SEED {
            CandidateKind::Literal
        } else {
            CandidateKind::Word
        },
        source: CandidateSource::Local,
    })
    .collect();
    let model = vec![
        row("日本語力", None, 0.55),
        row("日本語訳", None, 0.5),
        row("日本語版", None, 0.45),
        row(SENTENCE, None, 0.4),
        row("日本語の資料を送ります。", None, 0.35),
        row("日本語を練習したいです。", None, 0.3),
    ];
    let (once, accepted) = merge_predictions("ja", SEED, local.clone(), model.clone(), 12);
    let (again, _) = merge_predictions("ja", SEED, local, model, 12);
    assert!(accepted);
    assert_eq!(once, again);
    assert_eq!(once.len(), 12);
    assert_eq!(once[0].text, "日本語");
    assert_eq!(once[0].source, CandidateSource::Local);
    for kind in [CandidateKind::Word, CandidateKind::Sentence] {
        let model_rows: Vec<_> = once
            .iter()
            .filter(|row| row.source == CandidateSource::Model && row.kind == kind)
            .collect();
        assert!(
            model_rows
                .windows(2)
                .all(|pair| pair[0].score >= pair[1].score)
        );
    }
    assert!(
        once.iter()
            .all(|row| row.score.is_finite() && (0.0..=100.0).contains(&row.score))
    );
    let unique: std::collections::BTreeSet<_> = once.iter().map(|row| &row.text).collect();
    assert_eq!(unique.len(), once.len());
}

#[test]
fn a_real_worker_batch_infers_only_the_bounded_nominal_extension_not_neighboring_clauses() {
    let replies = [
        "日本語力",
        "明日出発",
        "日本語の力",
        "日本語 力",
        "日本語力。",
        "日本語を話す",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, text)| row(text, None, 0.6 - index as f32 * 0.05))
    .collect();
    let (_, engine) = run_worker(replies);
    assert_eq!(
        find(engine.candidates(), "日本語力").kind,
        CandidateKind::Word
    );
    for text in [
        "明日出発",
        "日本語の力",
        "日本語 力",
        "日本語力。",
        "日本語を話す",
    ] {
        assert_eq!(
            find(engine.candidates(), text).kind,
            CandidateKind::Sentence,
            "{text}"
        );
    }
    assert!(engine.candidates().len() <= 12);
    assert_eq!(engine.snapshot().seed_text, SEED);
    assert!(engine.snapshot().committed_text.is_empty());
}

// Frozen adapter rows from the subsequent authorized Astra Japanese recheck.
const CAPTURED_ASTRA_RECHECK: &str = r#"[{"text":"日本語入力","score_bias":0.550000011920929,"kind":null},{"text":"日本語で文章を書きます。","score_bias":0.5,"kind":null}]"#;

#[test]
fn the_additional_astra_visible_reply_replays_input_compound_and_sentence_through_the_worker() {
    let replies: Vec<PredictionCandidate> = serde_json::from_str(CAPTURED_ASTRA_RECHECK).unwrap();
    assert!(replies.iter().all(|reply| reply.kind.is_none()));
    let (before, engine) = run_worker(replies);
    let after = engine.candidates();
    assert_eq!(after[0].text, before[0].text);
    assert_eq!(after[0].source, CandidateSource::Local);
    let word = find(after, "日本語入力");
    assert_eq!(word.kind, CandidateKind::Word);
    assert_eq!(word.source, CandidateSource::Model);
    close(word.score, 90.0 + 0.55 * 6.0);
    let sentence = find(after, "日本語で文章を書きます。");
    assert_eq!(sentence.kind, CandidateKind::Sentence);
    assert_eq!(sentence.source, CandidateSource::Model);
    close(sentence.score, 82.0 + 0.5 * 6.0);
    let retained_local: Vec<_> = after
        .iter()
        .filter(|row| row.source == CandidateSource::Local)
        .map(|row| &row.text)
        .collect();
    assert_eq!(
        retained_local,
        before.iter().map(|row| &row.text).collect::<Vec<_>>()
    );
    assert!(after.len() <= 12);
    assert_eq!(engine.snapshot().seed_text, SEED);
    assert!(engine.snapshot().committed_text.is_empty());
    assert_eq!(classify("ja", SEED, "日本語入力"), CandidateKind::Sentence);
}

#[test]
fn known_input_domain_heads_generalize_only_with_whole_dictionary_object_stems() {
    for (stem, head) in [
        ("日本語", "入力"),
        ("日本語", "変換"),
        ("漢字", "入力"),
        ("漢字", "変換"),
        ("音声", "入力"),
        ("音声", "変換"),
    ] {
        assert_eq!(classify("ja", SEED, stem), CandidateKind::Word);
        assert_eq!(classify("ja", SEED, head), CandidateKind::Word);
        let text = format!("{stem}{head}");
        for language in ["ja", "ja-JP"] {
            let rows = merged(language, vec![row(&text, None, 0.45)]);
            let inferred = find(&rows, &text);
            assert_eq!(inferred.kind, CandidateKind::Word, "{language}: {text}");
            assert_eq!(inferred.source, CandidateSource::Model);
            close(inferred.score, 90.0 + 0.45 * 6.0);
        }
    }
}

#[test]
fn input_heads_do_not_turn_temporal_clauses_unknown_objects_or_recursive_segments_into_words() {
    for text in [
        "明日出発",
        "今日帰宅",
        "明日入力",
        "今日変換",
        "昨日入力",
        "今夜入力",
        "現在変換",
        "来週入力",
        "情報入力",
        "時間入力",
        "会議変換",
        "予約入力",
        "未知語入力",
        "未知複合入力",
        "語入力",
        "日本語入力変換",
        "日本語入力版",
        "日本語日本語日本語入力",
        "日本語 入力",
        "日本語の入力",
        "日本語入力。",
        "日本語入力で文章を書く",
        "にほんご入力",
        "音声を入力する",
        "漢字に変換する",
    ] {
        let rows = merged("ja", vec![row(text, None, 0.45)]);
        assert_ne!(find(&rows, text).kind, CandidateKind::Word, "{text}");
    }
    for language in ["en", "zh-Hans"] {
        let rows = merged(language, vec![row("日本語入力", None, 0.45)]);
        assert_eq!(find(&rows, "日本語入力").kind, CandidateKind::Sentence);
    }
    let rows = merged(
        "ja",
        vec![row("日本語入力", Some(PredictionKind::Sentence), 0.45)],
    );
    assert_eq!(find(&rows, "日本語入力").kind, CandidateKind::Sentence);
}

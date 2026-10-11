//! Independent Japanese time, waiting and rescheduling fallback fixtures.
//! These finite presets do not read calendars or personal input, call a live
//! model, or claim general Japanese morphology and appointment management.
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
    lexicon::{EntryKind, builtin},
};

const WORDS: &[(&str, &str, &str)] = &[
    ("gozen", "ごぜん", "午前"),
    ("gogo", "ごご", "午後"),
    ("konshuu", "こんしゅう", "今週"),
    ("senshuu", "せんしゅう", "先週"),
    ("kesa", "けさ", "今朝"),
    ("kon'ya", "こんや", "今夜"),
    ("yuugata", "ゆうがた", "夕方"),
    ("jikoku", "じこく", "時刻"),
    ("jikantai", "じかんたい", "時間帯"),
    ("getsuyou", "げつよう", "月曜"),
    ("kayou", "かよう", "火曜"),
    ("suiyou", "すいよう", "水曜"),
    ("mokuyou", "もくよう", "木曜"),
    ("kin'you", "きんよう", "金曜"),
    ("doyou", "どよう", "土曜"),
    ("nichiyou", "にちよう", "日曜"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "午前",
        [
            "午前の予定を確認しましょう。",
            "午前に間に合わなければ、午後にしましょう。",
        ],
    ),
    (
        "午後",
        [
            "午後の都合はいかがですか。",
            "午後に会う時間を決めましょう。",
        ],
    ),
    (
        "今週",
        [
            "今週の空いている時間を教えてください。",
            "今週が難しければ、来週にしましょう。",
        ],
    ),
    (
        "先週",
        [
            "先週の予定を確認してください。",
            "先週の約束を別の日に変更したいです。",
        ],
    ),
    (
        "今朝",
        [
            "今朝の予定はまだ間に合いますか。",
            "今朝は少し遅れるので、先に始めてください。",
        ],
    ),
    (
        "今夜",
        [
            "今夜の待ち合わせは何時ですか。",
            "今夜が難しければ、明日にしましょう。",
        ],
    ),
    (
        "夕方",
        ["夕方の都合を教えてください。", "夕方に着いたら連絡します。"],
    ),
    (
        "時間帯",
        [
            "時間帯を変えても大丈夫ですか。",
            "時間帯が決まったら教えてください。",
        ],
    ),
];

fn blank_engine() -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime
}
fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = blank_engine();
    ime.seed(seed);
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    ime
}
fn check_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates().iter().any(|candidate| {
            candidate.text == seed && candidate.kind == CandidateKind::Literal
        })
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|candidate| { candidate.source == CandidateSource::Local })
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {text:?} {kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        });
    assert!(
        kind == CandidateKind::Literal || index < PAGE_SIZE,
        "not page one {text:?}, seed={:?}: {:?}",
        ime.snapshot().seed_text,
        ime.candidates()
    );
    index
}
fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let selected = index(ime, text, kind);
    commit_index_and_undo(ime, selected, text);
}
fn commit_index_and_undo(ime: &mut XRTabletImeEngine, selected: usize, text: &str) {
    assert_eq!(ime.candidates()[selected].text, text);
    assert_eq!(ime.candidates()[selected].source, CandidateSource::Local);
    ime.select_candidate(selected);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok);
    assert!(duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn time_preserves_all_250_previous_readings_and_126_contexts_in_full_order() {
    // Captured before this batch: all old kind/separator flags and all 251
    // authored sentences, including the courtesy and location expansions.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 250);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(250) {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for byte in entry
            .reading
            .bytes()
            .chain([0])
            .chain(entry.text.bytes())
            .chain([0])
            .chain(kind.bytes())
            .chain([u8::from(entry.require_separators)])
        {
            readings = (readings ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(readings, 0x4367500fc72bc5ca);
    assert!(ja.continuations().count() >= 126);
    let mut contexts = 0xcbf29ce484222325_u64;
    let mut previous_sentence_count = 0;
    for (context, sentences) in ja.continuations().take(126) {
        previous_sentence_count += sentences.len();
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(previous_sentence_count, 251);
    assert_eq!(contexts, 0xe73c880aafa5b9ca);
}

#[test]
fn time_additions_match_the_independent_authored_window() {
    let ja = builtin("ja").unwrap();
    assert_eq!(WORDS.len(), 16);
    assert_eq!(SENTENCES.len(), 8);
    assert!(ja.readings().len() >= 266);
    assert!(ja.continuations().count() >= 134);
    assert_eq!(
        ja.continuations()
            .take(134)
            .map(|(_, sentences)| sentences.len())
            .sum::<usize>(),
        267
    );
    for (entry, &(_, kana, word)) in ja.readings().iter().skip(250).take(16).zip(WORDS) {
        assert_eq!(entry.reading, kana);
        assert_eq!(entry.text, word);
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(126).take(8).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn complete_time_readings_offer_first_page_words_sentences_and_exact_single_commits() {
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let ime = engine(&seed);
            check_draft(&ime, &seed);
            commit_once_and_undo(&mut engine(&seed), word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            if let Some(&(_, sentences)) = SENTENCES.iter().find(|(text, _)| *text == word) {
                for sentence in sentences {
                    commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn specific_partial_time_readings_keep_words_and_both_authored_sentence_branches() {
    // Specific fixed prefixes; broad one-kana requests retain their old ranks
    // and do not promise every new word or every authored branch on page one.
    for (seed, word) in [
        ("goze", "午前"),
        ("gog", "午後"),
        ("konshu", "今週"),
        ("senshu", "先週"),
        ("kes", "今朝"),
        ("kon'y", "今夜"),
        ("yuugat", "夕方"),
        ("jikok", "時刻"),
        ("getsuyo", "月曜"),
        ("kayo", "火曜"),
        ("suiyo", "水曜"),
        ("mokuyo", "木曜"),
        ("kin'yo", "金曜"),
        ("doyo", "土曜"),
        ("nichiyo", "日曜"),
        ("ごぜ", "午前"),
        ("こんしゅ", "今週"),
        ("せんしゅ", "先週"),
        ("ゆうが", "夕方"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
        if let Some(&(_, sentences)) = SENTENCES.iter().find(|(text, _)| *text == word) {
            for sentence in sentences {
                commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn ambiguous_compound_time_partials_keep_old_choices_and_second_page_sentence() {
    // 時間 is complete in these fragments; its live た tail has valid old
    // completions too. Preserve that ambiguity and the six-row page while
    // proving both new sentences remain reachable and submit exactly once.
    for seed in ["jikanta", "JIKANTA", "じかんた"] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "時間た");
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Sentence);
        index(&ime, "時間助かりました", CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), "時間帯", CandidateKind::Word);
        commit_once_and_undo(
            &mut engine(seed),
            "時間帯を変えても大丈夫ですか。",
            CandidateKind::Sentence,
        );
        let second = "時間帯が決まったら教えてください。";
        let selected = ime
            .candidates()
            .iter()
            .position(|c| c.text == second && c.kind == CandidateKind::Sentence)
            .unwrap();
        assert!((PAGE_SIZE..12).contains(&selected));
        commit_index_and_undo(&mut ime, selected, second);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn time_horizontal_separators_and_explicit_n_boundaries_preserve_raw_spelling() {
    for (seed, word, sentence) in [
        ("go zen", "午前", "午前の予定を確認しましょう。"),
        ("GO\u{3000}GO", "午後", "午後の都合はいかがですか。"),
        (
            "こん しゅう",
            "今週",
            "今週の空いている時間を教えてください。",
        ),
        ("\tsen shuu\t", "先週", "先週の予定を確認してください。"),
        ("  ke sa", "今朝", "今朝の予定はまだ間に合いますか。"),
        ("KON’YA", "今夜", "今夜の待ち合わせは何時ですか。"),
        ("yuu gata", "夕方", "夕方の都合を教えてください。"),
        ("じかん たい", "時間帯", "時間帯を変えても大丈夫ですか。"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for seed in ["kin'you", "KIN’YOU", "きんよう"] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut engine(seed), "金曜", CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn time_adoption_space_restore_and_particles_remain_an_editable_draft() {
    for (reading, word, tail, sentence) in [
        ("gozen", "午前", "no", "午前の予定を確認しましょう。"),
        ("gogo", "午後", "NI", "午後に会う時間を決めましょう。"),
        (
            "konshuu",
            "今週",
            "の",
            "今週の空いている時間を教えてください。",
        ),
        ("senshuu", "先週", "no", "先週の予定を確認してください。"),
        (
            "kesa",
            "今朝",
            "HA",
            "今朝は少し遅れるので、先に始めてください。",
        ),
        ("kon'ya", "今夜", "no", "今夜の待ち合わせは何時ですか。"),
        ("yuugata", "夕方", "ni", "夕方に着いたら連絡します。"),
        ("jikantai", "時間帯", "を", "時間帯を変えても大丈夫ですか。"),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Pure-engine adoption equivalent, not a physical host-key receipt.
        let spaced = format!("{word} ");
        ime.seed(&spaced);
        check_draft(&ime, &spaced);
        index(&ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(&spaced), &spaced, CandidateKind::Literal);
        ime.seed(reading);
        check_draft(&ime, reading);
        index(&ime, word, CandidateKind::Word);
        let continued = format!("{word} {tail}");
        ime.seed(&continued);
        check_draft(&ime, &continued);
        commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(&continued), &continued, CandidateKind::Literal);
    }
}

#[test]
fn time_keeps_unknown_text_and_never_joins_control_boundaries() {
    let mut seeds = vec![
        "xyzgozen".to_owned(),
        "🙂午前の".into(),
        "https://kon'ya".into(),
        "午後nixyz".into(),
        "今週をが".into(),
        "夕方に🙂".into(),
    ];
    let boundaries = [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ];
    for boundary in boundaries {
        for (head, tail) in [("go", "zen"), ("午前", "no"), ("kon'", "ya")] {
            seeds.push(format!("{head}{boundary}{tail}"));
        }
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        check_draft(&ime, &seed);
        for candidate in ime.candidates() {
            for &(_, sentences) in SENTENCES {
                for sentence in sentences {
                    assert!(
                        !candidate.text.ends_with(sentence),
                        "manufactured {sentence:?} from {seed:?}: {:?}",
                        ime.candidates()
                    );
                }
            }
            for boundary in boundaries {
                if seed.contains(boundary) {
                    assert!(candidate.text.contains(boundary));
                }
            }
        }
        commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct Unavailable(LlmProviderError);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-time-synthetic-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        Err(self.0.clone())
    }
}

#[test]
fn time_local_candidates_survive_typed_synthetic_provider_failures() {
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (seed, word, sentence) in [
            ("gozen", "午前", "午前の予定を確認しましょう。"),
            ("kon'ya", "今夜", "今夜の待ち合わせは何時ですか。"),
            ("jikantai", "時間帯", "時間帯を変えても大丈夫ですか。"),
        ] {
            let mut ime = blank_engine();
            ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
            ime.seed(seed);
            check_draft(&ime, seed);
            index(&ime, word, CandidateKind::Word);
            index(&ime, sentence, CandidateKind::Sentence);
            let local = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(ime.prediction_error(), Some(&error));
            assert_eq!(ime.candidates(), local);
            check_draft(&ime, seed);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

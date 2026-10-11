//! Independent regression fixtures for authored Japanese tails and whole readings.
//! Expectations are fixed here, not generated from the production dictionary.
//! This is finite offline fallback, not general morphology or desktop acceptance.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const STUDY: &str = "日本語を勉強したいです。";
const DONE: &str = "完了したら連絡してください。";
const BUSY: [&str; 2] = [
    "準備中です、少しお待ちください。",
    "準備中なので、終わったら連絡します。",
];

const AUTHORED_PROGRESS: &[(&str, &str, &str)] = &[
    ("日本語を勉強shitai", "日本語を勉強したい", STUDY),
    ("nihongo wo benkyoushitai", "日本語を勉強したい", STUDY),
    ("NIHONGO WO BENKYOUSHITAI", "日本語を勉強したい", STUDY),
    ("にほんごをべんきょうしたい", "日本語を勉強したい", STUDY),
    ("日本語をべんきょうしたい", "日本語を勉強したい", STUDY),
    ("日本語を勉強したい", "日本語を勉強したい", STUDY),
    ("日本語を勉強shita", "日本語を勉強した", STUDY),
    ("完了shitara", "完了したら", DONE),
    ("kanryoushitara", "完了したら", DONE),
    ("KANRYOU SHITARA", "完了したら", DONE),
    ("かんりょうしたら", "完了したら", DONE),
    ("完了したら", "完了したら", DONE),
    ("完了shita", "完了した", DONE),
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn check_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates().iter().any(|candidate| {
            candidate.text == seed && candidate.kind == CandidateKind::Literal
        })
    );
    assert!(ime.candidates().iter().all(|candidate| {
        candidate.source == CandidateSource::Local && (0.0..=100.0).contains(&candidate.score)
    }));
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
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    if kind != CandidateKind::Literal {
        assert!(
            index < PAGE_SIZE,
            "not on page one: {text:?}, seed={:?}, candidates={:?}",
            ime.snapshot().seed_text,
            ime.candidates()
        );
    }
    index
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok && second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn short_location_readings_never_split_an_authored_conjugation_tail() {
    for &(seed, progress, sentence) in AUTHORED_PROGRESS {
        let ime = engine(seed);
        check_draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, progress, "{seed:?}");
        index(&ime, sentence, CandidateKind::Sentence);
        assert!(
            ime.candidates().iter().all(|candidate| {
                !candidate.text.contains("勉強下") && !candidate.text.contains("完了下")
            }),
            "{seed:?}: {:?}",
            ime.candidates()
        );
    }
}

#[test]
fn whole_reading_completion_is_not_split_again_into_a_longer_location_word() {
    for seed in ["junbichu", "JUNBICHU", "じゅんびちゅ", "junbi chu"] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, "準備中", CandidateKind::Word);
        for sentence in BUSY {
            index(&ime, sentence, CandidateKind::Sentence);
        }
        assert!(
            ime.candidates()
                .iter()
                .all(|candidate| candidate.text != "準備中央"),
            "{seed:?}: {:?}",
            ime.candidates()
        );
    }
}

#[test]
fn an_authored_tail_keeps_genuine_partial_word_completions_after_particles() {
    for seed in [
        "準備gadeki",
        "JYUNBI GADEKI",
        "じゅんびができ",
        "準備ができ",
    ] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, "準備ができれば", CandidateKind::Word);
        index(&ime, "準備ができたら連絡します。", CandidateKind::Sentence);
        commit_once_and_undo(&mut ime, "準備ができれば", CandidateKind::Word);
        commit_once_and_undo(
            &mut engine(seed),
            "準備ができたら連絡します。",
            CandidateKind::Sentence,
        );
    }
}

#[test]
fn unfinished_consonant_probes_do_not_reinterpret_an_authored_tail_as_shita() {
    for seed in [
        "完了shit",
        "KANRYOU SHIT",
        "日本語を勉強shit",
        "nihongo wo benkyoushit",
    ] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        assert!(
            ime.candidates().iter().all(|candidate| {
                !candidate.text.contains("勉強下") && !candidate.text.contains("完了下")
            }),
            "{seed:?}: {:?}",
            ime.candidates()
        );
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
    // A broad probe keeps prior word/sentence choices first; the independent
    // location noun remains explicitly selectable on the later page.
    let mut ime = engine("shit");
    check_draft(&ime, "shit");
    let selected = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == "下" && candidate.kind == CandidateKind::Word)
        .unwrap();
    assert!((PAGE_SIZE..12).contains(&selected));
    ime.select_candidate(selected);
    assert_eq!(ime.selected_completion_text(true), Some("下"));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok && result.text.as_deref() == Some("下"));
    assert_eq!(ime.snapshot().committed_text, "下");
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok && second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, "下");
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn the_new_short_location_words_remain_direct_first_page_choices() {
    for (seed, word) in [
        ("shita", "下"),
        ("SHITA", "下"),
        ("した", "下"),
        ("chuuou", "中央"),
        ("CHUUOU", "中央"),
        ("ちゅうおう", "中央"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        assert_eq!(ime.candidates()[0].text, word, "{seed:?}");
    }
}

#[test]
fn a_known_location_word_and_particle_can_still_be_followed_by_shita() {
    for (seed, expected) in [
        ("iriguchinoshita", "入口の下"),
        ("iriguchi no shita", "入口の下"),
        ("いりぐちのした", "入口の下"),
        ("入口noshita", "入口の下"),
        ("入口のshita", "入口の下"),
        ("入口の下", "入口の下"),
        ("出口noshita", "出口の下"),
        ("階段のshita", "階段の下"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, expected, "{seed:?}");
    }
}

#[test]
fn unknown_prefixes_and_conflicting_tails_cannot_recover_unrelated_authored_sentences() {
    for seed in [
        "xyz日本語を勉強shitai",
        "xyznihongo wo benkyoushitai",
        "前置き完了shitara",
        "🙂完了shitara",
        "https://kanryoushitara",
        "完了shitaraxyz",
        "日本語を勉強shitai🙂",
        "xyzjunbichu",
    ] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        for candidate in ime.candidates() {
            for sentence in [STUDY, DONE, BUSY[0], BUSY[1]] {
                assert!(
                    !candidate.text.ends_with(sentence),
                    "{seed:?} must not manufacture {sentence:?}: {:?}",
                    ime.candidates()
                );
            }
        }
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}

#[test]
fn line_boundaries_are_kept_instead_of_manufacturing_whole_readings_or_sentences() {
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (head, tail) in [
            ("日本語を勉強", "shitai"),
            ("kanryou", "shitara"),
            ("junbi", "chu"),
        ] {
            let seed = format!("{head}{boundary}{tail}");
            let mut ime = engine(&seed);
            check_draft(&ime, &seed);
            for candidate in ime.candidates() {
                assert!(
                    candidate.text.contains(boundary),
                    "lost line boundary for {seed:?}: {candidate:?}"
                );
                for sentence in [STUDY, DONE, BUSY[0], BUSY[1]] {
                    assert!(!candidate.text.ends_with(sentence));
                }
            }
            commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn complete_word_and_sentence_payloads_commit_once_and_have_exactly_one_undo() {
    for (seed, word) in [
        ("shita", "下"),
        ("chuuou", "中央"),
        ("junbichu", "準備中"),
        ("iriguchi", "入口"),
    ] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut ime, word, CandidateKind::Word);
    }
    for (seed, sentence) in [
        ("日本語を勉強shitai", STUDY),
        ("完了shitara", DONE),
        ("junbichu", BUSY[0]),
        ("junbichu", BUSY[1]),
        ("iriguchinoshita", "入口の下"),
    ] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
    }
}

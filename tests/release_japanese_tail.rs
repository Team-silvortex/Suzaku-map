//! Independent release regressions for already supported Japanese Word aliases.
//! Known Katakana spelling must survive an authored grammatical tail; this does
//! not promise reverse lookup of arbitrary Katakana or a new whole-sentence
//! candidate in canonical spelling. Expectations are fixed, not lexicon-derived.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const STUDY: &str = "日本語を勉強したいです。";
const DONE: &str = "完了したら連絡してください。";
const HORIZONTAL: &[&str] = &[
    " ",
    "  ",
    "\t",
    "\u{a0}",
    "\u{3000}",
    "\u{2003}",
    " \t\u{a0}\u{3000}",
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

fn assert_local_draft(ime: &XRTabletImeEngine, seed: &str) {
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
    let found = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    // A crowded page must not make the exact raw draft unreachable.
    if kind != CandidateKind::Literal {
        assert!(found < PAGE_SIZE, "{text:?} was not on page one");
    }
    found
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

fn assert_progress(ime: &XRTabletImeEngine, seed: &str, expected: &str, kind: CandidateKind) {
    assert_local_draft(ime, seed);
    let primary = &ime.candidates()[0];
    assert_eq!(primary.text, expected, "{seed:?}");
    assert_eq!(primary.kind, kind, "{seed:?}");
    assert_eq!(primary.score, 100.0, "{seed:?}");
    // These aliases were known before the new standalone noun した -> 下.
    // No default conversion or vowel probe may reinterpret their grammar as 下.
    for forbidden in ["勉強下", "ベンキョウ下", "カンリョウ下"] {
        assert!(
            ime.candidates()
                .iter()
                .all(|candidate| !candidate.text.contains(forbidden)),
            "{seed:?} manufactured {forbidden:?}: {:?}",
            ime.candidates()
        );
    }
    index(ime, expected, kind);
}

#[test]
fn known_katakana_heads_keep_their_spelling_and_completed_grammar_tail() {
    for (seed, expected, kind) in [
        (
            "ニホンゴを勉強shitai",
            "ニホンゴを勉強したい",
            CandidateKind::Sentence,
        ),
        (
            "ニホンゴwobenkyoushitai",
            "ニホンゴを勉強したい",
            CandidateKind::Sentence,
        ),
        (
            "ニホンゴ wo benkyoushitai",
            "ニホンゴを勉強したい",
            CandidateKind::Sentence,
        ),
        (
            "カンリョウshitara",
            "カンリョウしたら",
            CandidateKind::Unspecified,
        ),
        (
            "カンリョウshita",
            "カンリョウした",
            CandidateKind::Unspecified,
        ),
    ] {
        let mut ime = engine(seed);
        assert_progress(&ime, seed, expected, kind);
        commit_once_and_undo(&mut ime, expected, kind);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn known_katakana_words_inside_the_draft_keep_their_own_written_form() {
    // 勉強/べんきょう is an existing Word, so ベンキョウ is a known alias
    // just like ニホンゴ. This is not arbitrary Katakana reverse conversion.
    for (seed, expected, kind) in [
        (
            "日本語をベンキョウshitai",
            "日本語をベンキョウしたい",
            CandidateKind::Sentence,
        ),
        (
            "ニホンゴをベンキョウshitai",
            "ニホンゴをベンキョウしたい",
            CandidateKind::Unspecified,
        ),
        (
            "日本語をベンキョウshita",
            "日本語をベンキョウした",
            CandidateKind::Sentence,
        ),
        (
            "ニホンゴをベンキョウshita",
            "ニホンゴをベンキョウした",
            CandidateKind::Unspecified,
        ),
    ] {
        let mut ime = engine(seed);
        assert_progress(&ime, seed, expected, kind);
        commit_once_and_undo(&mut ime, expected, kind);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn unfinished_consonant_probes_do_not_create_shita_nouns_after_known_aliases() {
    for (seed, expected) in [
        ("カンリョウshit", "カンリョウしt"),
        ("カンリョウ SHIT", "カンリョウしt"),
        ("ニホンゴを勉強shit", "ニホンゴを勉強しt"),
        ("日本語をベンキョウshit", "日本語をベンキョウしt"),
        ("ニホンゴをベンキョウshit", "ニホンゴをベンキョウしt"),
    ] {
        let mut ime = engine(seed);
        // An unfinished phonetic reading stays untyped, even with known kanji.
        assert_progress(&ime, seed, expected, CandidateKind::Unspecified);
        commit_once_and_undo(&mut ime, expected, CandidateKind::Unspecified);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn canonical_written_controls_keep_the_existing_authored_sentence_choices() {
    for (seed, progress, sentence) in [
        ("日本語を勉強shitai", "日本語を勉強したい", STUDY),
        ("nihongo wo benkyoushitai", "日本語を勉強したい", STUDY),
        ("日本語を勉強shita", "日本語を勉強した", STUDY),
        ("完了shitara", "完了したら", DONE),
        ("KANRYOU SHITARA", "完了したら", DONE),
        ("完了shita", "完了した", DONE),
    ] {
        let mut ime = engine(seed);
        assert_progress(&ime, seed, progress, CandidateKind::Sentence);
        index(&ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut ime, progress, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn adopted_known_aliases_and_word_completions_remain_editable_before_commit() {
    for (reading, adopted, continued, expected, kind) in [
        (
            "nihongo",
            "ニホンゴ",
            "ニホンゴwobenkyoushitai",
            "ニホンゴを勉強したい",
            CandidateKind::Sentence,
        ),
        (
            "kanryou",
            "カンリョウ",
            "カンリョウshitara",
            "カンリョウしたら",
            CandidateKind::Unspecified,
        ),
        (
            "ニホンゴwobenky",
            "ニホンゴを勉強",
            "ニホンゴを勉強shitai",
            "ニホンゴを勉強したい",
            CandidateKind::Sentence,
        ),
        (
            "日本語wobenky",
            "日本語を勉強",
            "日本語を勉強shitai",
            "日本語を勉強したい",
            CandidateKind::Sentence,
        ),
    ] {
        let mut ime = engine(reading);
        assert_local_draft(&ime, reading);
        ime.select_candidate(index(&ime, adopted, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(adopted));
        assert!(ime.snapshot().committed_text.is_empty());
        // The host adopts into its cumulative editable draft with seed(), not
        // append_seed() as a raw physical-key API. This is not a GTK/key gate.
        ime.seed(adopted);
        assert_local_draft(&ime, adopted);
        ime.seed(continued);
        assert_progress(&ime, continued, expected, kind);
        commit_once_and_undo(&mut ime, expected, kind);
        commit_once_and_undo(&mut engine(continued), continued, CandidateKind::Literal);
    }
}

#[test]
fn horizontal_spaces_and_ascii_case_preserve_known_alias_tails_and_exact_raw() {
    for &separator in HORIZONTAL {
        for (head, tail, expected, kind) in [
            (
                "ニホンゴを勉強",
                "SHITAI",
                "ニホンゴを勉強したい",
                CandidateKind::Sentence,
            ),
            (
                "日本語をベンキョウ",
                "shitai",
                "日本語をベンキョウしたい",
                CandidateKind::Sentence,
            ),
            (
                "ニホンゴをベンキョウ",
                "SHITAI",
                "ニホンゴをベンキョウしたい",
                CandidateKind::Unspecified,
            ),
            (
                "カンリョウ",
                "SHITARA",
                "カンリョウしたら",
                CandidateKind::Unspecified,
            ),
        ] {
            let seed = format!("{head}{separator}{tail}{separator}");
            let mut ime = engine(&seed);
            assert_progress(&ime, &seed, expected, kind);
            commit_once_and_undo(&mut ime, expected, kind);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn arbitrary_katakana_and_unknown_text_cannot_recover_known_alias_sentences() {
    for seed in [
        "テストniho",
        "ワカラナイniho",
        "ワカラナイshitara",
        "xyzニホンゴを勉強shitai",
        "🙂カンリョウshitara",
        "ニホンゴxyzbenky",
        "ニホンゴをベンキョウshitaixyz",
        "カンリョウshitaraxyz",
        "https://ニホンゴを勉強shitai",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        for candidate in ime.candidates() {
            for sentence in [STUDY, DONE] {
                assert!(
                    !candidate.text.ends_with(sentence),
                    "{seed:?} manufactured {sentence:?}: {candidate:?}"
                );
            }
            for forbidden in [
                "日本語",
                "日本語を勉強したい",
                "ニホンゴを勉強したい",
                "ニホンゴをベンキョウしたい",
                "完了したら",
                "カンリョウしたら",
            ] {
                assert_ne!(
                    candidate.text, forbidden,
                    "discarded unknown text in {seed:?}"
                );
            }
        }
        // No grammatical conversion guarantee is made for unknown aliases.
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}

#[test]
fn all_seven_line_boundaries_remain_significant_between_alias_and_tail() {
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (head, tail) in [
            ("ニホンゴを勉強", "shitai"),
            ("日本語をベンキョウ", "shitai"),
            ("ニホンゴをベンキョウ", "shitai"),
            ("カンリョウ", "shitara"),
        ] {
            let seed = format!("{head}{boundary}{tail}");
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            for candidate in ime.candidates() {
                assert!(
                    candidate.text.contains(boundary),
                    "lost boundary in {seed:?}: {candidate:?}"
                );
                for sentence in [STUDY, DONE] {
                    assert!(
                        !candidate.text.ends_with(sentence),
                        "{seed:?} manufactured {sentence:?}: {candidate:?}"
                    );
                }
            }
            commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
        }
    }
}

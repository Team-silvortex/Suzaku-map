//! Existing authored sentence progress, not new vocabulary or page-one quotas.
//! The coupon is a positive control; the retry probe exposed the missing path.
use std::collections::HashSet;
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
};

struct Progress {
    adopted: &'static str,
    reading: &'static str,
    primary_tail: &'static str,
    word: &'static str,
    sentence_tail: &'static str,
}

const CASES: &[Progress] = &[
    Progress {
        adopted: "优惠券还在",
        reading: "you'xi",
        primary_tail: "有西",
        word: "有效期",
        sentence_tail: "有效期内。",
    },
    Progress {
        adopted: "登录失败后可以",
        reading: "zai'shi'yi",
        primary_tail: "在释义",
        word: "再试一次",
        sentence_tail: "再试一次。",
    },
];
const PADDING: [&str; 6] = ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    ime
}

fn spellings(reading: &str) -> [String; 5] {
    [
        reading.replace('\'', ""),
        reading.replace('\'', " "),
        reading.to_owned(),
        reading.replace('\'', "\t"),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}

fn assert_local_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 2 * PAGE_SIZE);
    assert!(
        ime.candidates()
            .iter()
            .all(|choice| choice.source == CandidateSource::Local)
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|choice| &choice.text)
            .collect::<HashSet<_>>()
            .len()
    );
    candidate_index(ime, seed, CandidateKind::Literal);
}

fn candidate_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = ime
        .candidates()
        .iter()
        .position(|choice| choice.text == text && choice.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    assert!(index < 2 * PAGE_SIZE);
    index
}

fn assert_progress(ime: &XRTabletImeEngine, case: &Progress, prefix: &str, padding: &str) {
    // The old primary stays pinned, even when a later verified Word can finish
    // an authored sentence. Neither completion is required on the first page.
    assert_eq!(
        ime.candidates()[0].text,
        format!("{prefix}{}{padding}{}", case.adopted, case.primary_tail)
    );
    assert_eq!(ime.candidates()[0].kind, CandidateKind::Sentence);
    assert_eq!(ime.candidates()[0].score, 100.0);
    candidate_index(
        ime,
        &format!("{prefix}{}{padding}{}", case.adopted, case.word),
        CandidateKind::Word,
    );
    candidate_index(
        ime,
        &format!("{prefix}{}{padding}{}", case.adopted, case.sentence_tail),
        CandidateKind::Sentence,
    );
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(candidate_index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert_eq!(second.text, None);
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn adopted_partial_progress_keeps_words_and_fixed_sentences_in_five_spellings() {
    for case in CASES {
        for padding in PADDING {
            for tail in spellings(case.reading) {
                let seed = format!("{}{padding}{tail}", case.adopted);
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                assert_progress(&ime, case, "", padding);
            }
        }
    }
}

#[test]
fn long_unicode_frozen_prefixes_retain_the_same_authored_sentence_progress() {
    let prefix = "前文 café 😀。  ".repeat(40);
    for case in CASES {
        for padding in PADDING {
            for tail in spellings(case.reading) {
                let seed = format!("{prefix}{}{padding}{tail}", case.adopted);
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                assert_progress(&ime, case, &prefix, padding);
                assert!(
                    ime.candidates()
                        .iter()
                        .all(|choice| choice.text.starts_with(&prefix))
                );
            }
        }
    }
}

#[test]
fn sentence_progress_uses_scalar_not_byte_boundaries_at_the_local_window_limit() {
    for case in CASES {
        for size in [255, 256, 257, 512] {
            for padding in PADDING {
                let tail = case.reading.replace('\'', "");
                let suffix = format!("{}{padding}{tail}", case.adopted);
                let prefix = "😀前".repeat((size - suffix.chars().count()) / 2);
                let prefix = format!(
                    "{prefix}{}",
                    "前".repeat(size - suffix.chars().count() - prefix.chars().count())
                );
                let seed = format!("{prefix}{suffix}");
                assert_eq!(seed.chars().count(), size);
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                assert_progress(&ime, case, &prefix, padding);
                assert!(
                    ime.candidates()
                        .iter()
                        .all(|choice| choice.text.starts_with(&prefix))
                );
            }
        }
    }
}

#[test]
fn both_pages_support_one_word_sentence_or_literal_commit_and_one_undo() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for case in CASES {
            for padding in PADDING {
                let seed = format!("{prefix}{}{padding}{}", case.adopted, case.reading);
                for (text, kind) in [
                    (
                        format!("{prefix}{}{padding}{}", case.adopted, case.word),
                        CandidateKind::Word,
                    ),
                    (
                        format!("{prefix}{}{padding}{}", case.adopted, case.sentence_tail),
                        CandidateKind::Sentence,
                    ),
                    (seed.clone(), CandidateKind::Literal),
                ] {
                    let mut ime = engine(&seed);
                    assert_local_draft(&ime, &seed);
                    commit_once_and_undo(&mut ime, &text, kind);
                }
            }
        }
    }
}

#[test]
fn adopting_a_verified_progress_word_keeps_the_sentence_editable_without_a_commit() {
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for case in CASES {
            for padding in PADDING {
                let seed = format!("{prefix}{}{padding}{}", case.adopted, case.reading);
                let word = format!("{prefix}{}{padding}{}", case.adopted, case.word);
                let mut ime = engine(&seed);
                ime.select_candidate(candidate_index(&ime, &word, CandidateKind::Word));
                assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
                assert!(ime.snapshot().committed_text.is_empty());
                for trailing in ["", " ", "  ", "\u{3000}"] {
                    let adopted = format!("{word}{trailing}");
                    ime.seed(&adopted);
                    assert_local_draft(&ime, &adopted);
                    let expected = format!(
                        "{adopted}{}",
                        case.sentence_tail.strip_prefix(case.word).unwrap()
                    );
                    candidate_index(&ime, &expected, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn sentence_completion_never_corrects_already_adopted_wrong_han() {
    for (case, wrong) in [(&CASES[0], "优惠卷还在"), (&CASES[1], "登陆失败后可以")] {
        for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
            for tail in spellings(case.reading) {
                let seed = format!("{prefix}{wrong}{tail}");
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                let corrected = format!("{prefix}{}{}", case.adopted, case.sentence_tail);
                assert!(
                    !ime.candidates()
                        .iter()
                        .any(|choice| choice.text == corrected)
                );
                assert!(
                    ime.candidates()
                        .iter()
                        .all(|choice| choice.text.starts_with(&format!("{prefix}{wrong}")))
                );
            }
        }
    }
}

#[test]
fn invalid_splits_unknown_tails_and_line_boundaries_cannot_manufacture_progress() {
    for (case, invalid) in [
        (&CASES[0], ["yo u xi", "you x i", "zzqvjy", "?? zzqvjy"]),
        (
            &CASES[1],
            ["za i shi yi", "zai sh i yi", "zzqvjy", "?? zzqvjy"],
        ),
    ] {
        for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
            for tail in invalid {
                let seed = format!("{prefix}{}{tail}", case.adopted);
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                let expected = format!("{prefix}{}{}", case.adopted, case.sentence_tail);
                assert!(
                    !ime.candidates()
                        .iter()
                        .any(|choice| choice.text == expected)
                );
            }
            for line in [
                "\n", "\r", "\u{000b}", "\u{000c}", "\u{0085}", "\u{2028}", "\u{2029}",
            ] {
                let seed = format!("{prefix}{}{line}{}", case.adopted, case.reading);
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                for gap in ["", line] {
                    let expected = format!("{prefix}{}{gap}{}", case.adopted, case.sentence_tail);
                    assert!(
                        !ime.candidates()
                            .iter()
                            .any(|choice| choice.text == expected)
                    );
                }
            }
        }
    }
}

#[test]
fn protected_url_identifier_and_address_tokens_do_not_become_authored_contexts() {
    for case in CASES {
        for prefix in [
            "https://example.invalid/",
            "file_name_",
            "mail@example.invalid/",
            "folder\\",
        ] {
            for frozen in [String::new(), "😀前".repeat(150)] {
                let seed = format!("{frozen}{prefix}{}{}", case.adopted, case.reading);
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                let expected = format!("{frozen}{prefix}{}{}", case.adopted, case.sentence_tail);
                assert!(
                    !ime.candidates()
                        .iter()
                        .any(|choice| choice.text == expected)
                );
            }
        }
    }
}

#[test]
fn exact_time_homophones_keep_the_old_first_page_guard() {
    for seed in spellings("shi'jian") {
        let ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        assert_eq!(ime.candidates()[0].text, "时间");
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        assert_eq!(ime.candidates()[0].score, 100.0);
        let index = candidate_index(&ime, "世间", CandidateKind::Word);
        assert!(index < PAGE_SIZE);
        assert!(!ime.candidates().iter().any(|choice| {
            choice.kind == CandidateKind::Sentence && choice.text.starts_with("时间安排")
        }));
    }
}

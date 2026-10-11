//! A literal acronym before an explicit Chinese clause boundary must not
//! prevent the following known Pinyin from converting in a short draft.
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const MIXED_PREFIXES: [&str; 3] = ["已确认 API。", "已确认 Rust2026，", "前文 café 😀；"];
const HORIZONTAL: [&str; 6] = ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"];

fn spellings(reading: &str) -> [String; 5] {
    [
        reading.replace('\'', ""),
        reading.replace('\'', " "),
        reading.to_owned(),
        reading.replace('\'', "\t"),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn assert_choice(ime: &XRTabletImeEngine, expected: &str, kind: CandidateKind) {
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
    );
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == expected && c.kind == kind),
        "missing {kind:?} {expected:?}: {:?}",
        ime.candidates()
    );
}

#[test]
fn long_mixed_prefix_already_preserves_acronym_and_converts_the_next_clause() {
    let prefix = format!("{}已确认 API。", "前文。".repeat(90));
    let seed = format!("{prefix}nihao");
    let ime = engine(&seed);
    assert_choice(&ime, &format!("{prefix}你好"), CandidateKind::Word);
    assert_choice(&ime, &seed, CandidateKind::Literal);
}

#[test]
fn short_mixed_prefix_preserves_acronym_and_converts_the_next_clause() {
    let prefix = "已确认 API。";
    let seed = format!("{prefix}nihao");
    let ime = engine(&seed);
    assert_choice(&ime, &format!("{prefix}你好"), CandidateKind::Word);
    assert_choice(&ime, &seed, CandidateKind::Literal);
}

fn assert_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_choice(ime, seed, CandidateKind::Literal);
}

fn choice_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    assert_choice(ime, text, kind);
    ime.candidates()
        .iter()
        .position(|choice| choice.text == text && choice.kind == kind)
        .unwrap()
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(choice_index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty());
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert_eq!(second.text, None);
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn explicit_cjk_boundaries_preserve_mixed_case_and_horizontal_padding() {
    for prefix in MIXED_PREFIXES {
        for gap in HORIZONTAL {
            for reading in spellings("ni'hao") {
                let seed = format!("{prefix}{gap}{reading}");
                let ime = engine(&seed);
                assert_draft(&ime, &seed);
                assert_choice(&ime, &format!("{prefix}{gap}你好"), CandidateKind::Word);
                assert_choice(
                    &ime,
                    &format!("{prefix}{gap}你好，很高兴认识你。"),
                    CandidateKind::Sentence,
                );
                assert!(
                    ime.candidates()
                        .iter()
                        .all(|choice| choice.text.starts_with(prefix))
                );
            }
        }
    }
}

#[test]
fn known_partial_words_keep_existing_authored_sentences_after_mixed_prefixes() {
    for prefix in MIXED_PREFIXES {
        for gap in ["", " ", "\u{3000}"] {
            for reading in spellings("zhong'w") {
                let seed = format!("{prefix}{gap}{reading}");
                let ime = engine(&seed);
                assert_draft(&ime, &seed);
                assert_choice(&ime, &format!("{prefix}{gap}中文"), CandidateKind::Word);
                assert_choice(
                    &ime,
                    &format!("{prefix}{gap}中文输入很方便。"),
                    CandidateKind::Sentence,
                );
            }
        }
    }
}

#[test]
fn mixed_prefixes_retain_adopted_han_and_verified_partial_sentence_progress() {
    for prefix in MIXED_PREFIXES {
        for gap in ["", " ", "\t"] {
            for (adopted, reading, word, sentence) in [
                ("发音", "ke y", "可以", "可以再示范一下吗？"),
                ("优惠券还在", "YOU XI", "有效期", "有效期内。"),
                ("登录失败后可以", "zai'shi'yi", "再试一次", "再试一次。"),
            ] {
                let seed = format!("{prefix}{adopted}{gap}{reading}");
                let ime = engine(&seed);
                assert_draft(&ime, &seed);
                assert_choice(
                    &ime,
                    &format!("{prefix}{adopted}{gap}{word}"),
                    CandidateKind::Word,
                );
                assert_choice(
                    &ime,
                    &format!("{prefix}{adopted}{gap}{sentence}"),
                    CandidateKind::Sentence,
                );
            }
        }
    }
}

#[test]
fn adopted_han_can_end_a_mixed_literal_prefix_without_a_new_clause_punctuation() {
    for prefix in ["已确认 API 我想学习", "Rust2026 café 😀 我想学习"] {
        for gap in ["", " ", "\u{3000}"] {
            for reading in ["zhong w", "ZHONG W"] {
                let seed = format!("{prefix}{gap}{reading}");
                let ime = engine(&seed);
                assert_draft(&ime, &seed);
                assert_choice(&ime, &format!("{prefix}{gap}中文"), CandidateKind::Word);
                // Authored contexts are exact substring matches: a space
                // inside 我想学习中文 does not match that entire context.
                // Its shorter 中文 context still retains its own fixed sentence.
                let sentence = if gap.is_empty() {
                    "中文，请多指教。"
                } else {
                    "中文输入很方便。"
                };
                assert_choice(
                    &ime,
                    &format!("{prefix}{gap}{sentence}"),
                    CandidateKind::Sentence,
                );
            }
        }
    }
}

#[test]
fn mixed_unicode_drafts_keep_the_same_choices_at_255_256_and_257_scalars() {
    for size in [255, 256, 257] {
        for (tail, word, sentence) in [
            ("nihao", "你好", "你好，很高兴认识你。"),
            ("zhong w", "中文", "中文输入很方便。"),
            (
                "登录失败后可以zaishiyi",
                "登录失败后可以再试一次",
                "登录失败后可以再试一次。",
            ),
        ] {
            let marker = " API Rust2026 café 😀。";
            let padding = size - marker.chars().count() - tail.chars().count();
            let prefix = format!("{}{marker}", "前".repeat(padding));
            let seed = format!("{prefix}{tail}");
            assert_eq!(seed.chars().count(), size);
            assert!(
                seed.len() > size,
                "fixture must distinguish bytes from scalars"
            );
            let ime = engine(&seed);
            assert_draft(&ime, &seed);
            assert_choice(&ime, &format!("{prefix}{word}"), CandidateKind::Word);
            assert_choice(
                &ime,
                &format!("{prefix}{sentence}"),
                CandidateKind::Sentence,
            );
            assert!(
                ime.candidates()
                    .iter()
                    .all(|choice| choice.text.starts_with(&prefix))
            );
        }
    }
}

#[test]
fn mixed_word_sentence_and_literal_commit_once_with_one_committed_text_undo() {
    for prefix in [
        "已确认 API Rust2026 café 😀。".to_owned(),
        "前文。".repeat(90),
    ] {
        let seed = format!("{prefix}登录失败后可以 ZAI SHI YI");
        for (text, kind) in [
            (
                format!("{prefix}登录失败后可以 再试一次"),
                CandidateKind::Word,
            ),
            (
                format!("{prefix}登录失败后可以 再试一次。"),
                CandidateKind::Sentence,
            ),
            (seed.clone(), CandidateKind::Literal),
        ] {
            let mut ime = engine(&seed);
            assert_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, &text, kind);
        }
    }
}

#[test]
fn adopting_a_mixed_partial_word_keeps_space_and_next_reading_editable() {
    for prefix in MIXED_PREFIXES {
        let seed = format!("{prefix}发音 ke y");
        let word = format!("{prefix}发音 可以");
        let mut ime = engine(&seed);
        assert_draft(&ime, &seed);
        ime.select_candidate(choice_index(&ime, &word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
        assert!(ime.snapshot().committed_text.is_empty());
        // Reseed the visible choice as the host's adoption operation does.
        // This is a core workflow, not delivery through a physical keyboard.
        ime.seed(&word);
        for gap in [" ", "\u{3000}"] {
            let continued = format!("{word}{gap}zai");
            ime.seed(&continued);
            assert_draft(&ime, &continued);
            assert_choice(
                &ime,
                &format!("{word}{gap}再示范一下吗？"),
                CandidateKind::Sentence,
            );
        }
    }
}

#[test]
fn existing_whole_reading_primaries_are_not_replaced_by_mixed_tail_routing() {
    for (reading, primary) in [("ni'hao", "你好"), ("shi'jian", "时间"), ("zai", "在")] {
        for seed in spellings(reading) {
            let ime = engine(&seed);
            assert_draft(&ime, &seed);
            assert_eq!(ime.candidates()[0].text, primary, "{seed:?}");
            assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        }
    }
    let ime = engine("shijian");
    assert_choice(&ime, "世间", CandidateKind::Word);
}

#[test]
fn incorrect_internal_splits_and_unknown_tokens_do_not_manufacture_mixed_completions() {
    for prefix in MIXED_PREFIXES {
        for tail in ["z hong w", "zh ong w", "unknownprefixzhongw", "zzqvjy"] {
            let seed = format!("{prefix}{tail}");
            let ime = engine(&seed);
            assert_draft(&ime, &seed);
            for forbidden in ["中文", "中文输入很方便。"] {
                assert!(
                    !ime.candidates()
                        .iter()
                        .any(|choice| choice.text == format!("{prefix}{forbidden}")),
                    "{seed:?}: {:?}",
                    ime.candidates()
                );
            }
        }
        for tail in [
            "za i shi yi",
            "zai sh i yi",
            "unknownprefixzaishiyi",
            "zzqvjy",
        ] {
            let seed = format!("{prefix}登录失败后可以{tail}");
            let ime = engine(&seed);
            assert_draft(&ime, &seed);
            for forbidden in ["再试一次", "再试一次。"] {
                assert!(
                    !ime.candidates().iter().any(|choice| {
                        choice.text == format!("{prefix}登录失败后可以{forbidden}")
                    }),
                    "{seed:?}: {:?}",
                    ime.candidates()
                );
            }
        }
    }
}

#[test]
fn protected_token_syntax_is_not_hidden_behind_a_han_tail_boundary() {
    for prefix in [
        "已确认 API。".to_owned(),
        "已确认 Rust2026。".to_owned(),
        "前文 café 😀。".repeat(40),
    ] {
        for token in [
            "https://example.invalid/",
            "file_name_",
            "mail@example.invalid/",
            "folder\\",
        ] {
            for (tail, forbidden) in [
                ("nihao", "你好"),
                ("登录失败后可以zaishiyi", "登录失败后可以再试一次。"),
            ] {
                // No whitespace inside the protected token: whitespace would
                // legitimately start another token and weaken this fixture.
                let seed = format!("{prefix}{token}{tail}");
                let ime = engine(&seed);
                assert_draft(&ime, &seed);
                assert!(
                    !ime.candidates()
                        .iter()
                        .any(|choice| choice.text == format!("{prefix}{token}{forbidden}")),
                    "{seed:?}: {:?}",
                    ime.candidates()
                );
            }
        }
    }
}

#[test]
fn vertical_boundaries_are_not_removed_or_crossed_to_form_one_authored_sentence() {
    for prefix in MIXED_PREFIXES {
        for line in [
            "\n", "\r", "\u{000b}", "\u{000c}", "\u{0085}", "\u{2028}", "\u{2029}",
        ] {
            let seed = format!("{prefix}登录失败后可以{line}zaishiyi");
            let ime = engine(&seed);
            assert_draft(&ime, &seed);
            for gap in ["", line] {
                assert!(
                    !ime.candidates().iter().any(|choice| {
                        choice.text == format!("{prefix}登录失败后可以{gap}再试一次。")
                    }),
                    "{seed:?}: {:?}",
                    ime.candidates()
                );
            }
        }
    }
}

#[test]
fn closing_quotes_remain_literal_and_do_not_become_discardable_reading_padding() {
    for prefix in MIXED_PREFIXES
        .into_iter()
        .map(str::to_owned)
        .chain([String::new(), "前文 API Rust2026 café 😀。".repeat(40)])
    {
        let seed = format!("{prefix}nihao'");
        let ime = engine(&seed);
        assert_draft(&ime, &seed);
        assert_choice(&ime, &format!("{prefix}你好'"), CandidateKind::Sentence);
        assert!(
            !ime.candidates()
                .iter()
                .any(|choice| choice.text == format!("{prefix}你好"))
        );

        for adopted in ["", "登录失败后可以"] {
            for padding in ["", "  ", "\t"] {
                let seed = format!("{prefix}{adopted}zaishiyi'{padding}");
                let ime = engine(&seed);
                assert_draft(&ime, &seed);
                assert!(
                    ime.candidates()
                        .iter()
                        .all(|choice| choice.text.contains('\'')),
                    "lost closing quote in {seed:?}: {:?}",
                    ime.candidates()
                );
            }
        }
    }
}

#[test]
fn tone_positions_and_literal_decimal_clock_and_grouping_digits_stay_distinct() {
    for prefix in MIXED_PREFIXES {
        for (reading, converted, kind) in [
            ("ni3 hao3", "你好", CandidateKind::Word),
            ("NI3 HAO3", "你好", CandidateKind::Word),
            ("ni3hao3!", "你好!", CandidateKind::Sentence),
            ("hao3.5", "好3.5", CandidateKind::Sentence),
            ("hao3:30", "好3:30", CandidateKind::Sentence),
            ("hao3,000", "好3,000", CandidateKind::Sentence),
            ("hao30!", "好30!", CandidateKind::Sentence),
            ("hao6!", "好6!", CandidateKind::Sentence),
        ] {
            let seed = format!("{prefix}{reading}");
            let expected = format!("{prefix}{converted}");
            let mut ime = engine(&seed);
            assert_draft(&ime, &seed);
            assert_choice(&ime, &expected, kind);
            commit_once_and_undo(&mut ime, &expected, kind);
        }
    }
}

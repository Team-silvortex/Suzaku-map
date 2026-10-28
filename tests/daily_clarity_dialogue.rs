//! Authored clarification language, not general reasoning or a live-model benchmark.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/presence_resource.rs"]
mod presence_resource;
#[path = "support/release_storage_context.rs"]
mod release_storage_context;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::BTreeSet;
use suzaku_map::{
    ime::candidate_mix::CandidateKind,
    languages::{english::is_known_english_word, japanese::romaji_to_hiragana},
    lexicon::builtin,
};

const SUITE: Suite = Suite { marker: "CLARITY" };
// Frozen independently of the production resource and candidate output.
const EN: &[EnglishCase] = &[
    (
        "before we start, i want to confirm",
        ["what", "where"],
        ["wha", "wher"],
        [
            "before we start, i want to confirm what you need.",
            "before we start, i want to confirm where we should meet.",
        ],
    ),
    (
        "by later, do you mean",
        ["tonight", "tomorrow"],
        ["tonigh", "tomorro"],
        [
            "by later, do you mean tonight after dinner?",
            "by later, do you mean tomorrow morning?",
        ],
    ),
    (
        "for our next meeting, would you prefer",
        ["morning", "afternoon"],
        ["mornin", "afternoo"],
        [
            "for our next meeting, would you prefer morning or afternoon?",
            "for our next meeting, would you prefer afternoon or evening?",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "fen'qi",
        "fen'q",
        "分歧",
        [
            "分歧主要在先做哪一步，我们再对一遍。",
            "分歧可以慢慢说清楚，先听听你的想法。",
        ],
    ),
    (
        "bu'zhou",
        "bu'zho",
        "步骤",
        [
            "步骤可以分开说，我想先弄清第一步。",
            "步骤的先后顺序我还想再确认一下。",
        ],
    ),
    (
        "yi'wen",
        "yi'we",
        "疑问",
        [
            "疑问主要在第二点，可以再解释一下吗？",
            "疑问还没有解决，我们换个例子再说一遍吧。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "hitsuyou",
        "hitsuyo",
        "必要",
        [
            "必要な条件を、先に二つだけ教えてもらえますか。",
            "必要なら、別の例で説明してもらえますか。",
        ],
    ),
    (
        "tejun",
        "teju",
        "手順",
        [
            "手順は、最初のところから順番に教えてください。",
            "手順の違いを、もう一度一緒に確認しましょう。",
        ],
    ),
    (
        "mikakunin",
        "mikakuni",
        "未確認",
        [
            "未確認の点は、あとでまとめて質問します。",
            "未確認の部分があるので、返事は少し待ってください。",
        ],
    ),
];

fn sha_text(text: &str) -> String {
    digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn clarity_preserves_exact_previous_resource_bytes_flags_order_and_authors() {
    let mut en = presence_resource::without_presence(include_str!("../data/lexicons/en.json"));
    let layer_first = "\n    {\n      \"id\": \"daily_essentials_dialogue\",";
    let layer_after = "\n    }\n  ],\n  \"phrase_endings\": [";
    assert_eq!(en.matches(layer_first).count(), 1);
    let start = en.find(layer_first).unwrap();
    assert_eq!(en[start..].matches(layer_after).count(), 1);
    let end = start + en[start..].find(layer_after).unwrap();
    let mut layer = en[start..end].to_owned();
    for (first, after) in [
        (
            ",\n        [\n          \"before we start, i want to confirm\",",
            "\n      ],\n      \"sentences\": [",
        ),
        (
            ",\n        \"before we start, i want to confirm what you need.\"",
            "\n      ]",
        ),
    ] {
        if layer.contains(first) {
            assert_eq!(layer.matches(first).count(), 1);
            let from = layer.find(first).unwrap();
            assert_eq!(layer[from..].matches(after).count(), 1);
            let to = from + layer[from..].find(after).unwrap();
            assert!(from < to);
            layer.replace_range(from..to, "");
        }
    }
    en.replace_range(start..end, &layer);
    assert_eq!(
        sha_text(&en),
        "ad3fbdcaaadcb69a6d6056d82bc9f57af4cc3f89f140620a43f241205e5d3667"
    );
    for (raw, first, hash) in [
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            ",\n    [\n      \"分歧\",",
            "119bddcbce9d69a55e01038f52cfc566dd418997dcda248170f50e87f3e24f34",
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            ",\n    [\n      \"必要\",",
            "083a8152b3e71594e60ac005d4e590d79cce815807582b6fea89480bb74ac693",
        ),
    ] {
        SUITE.preserve_resource(
            &release_storage_context::restore_pre_release_contexts(raw),
            first,
            "\n  ]\n}",
            hash,
        );
    }
}

#[test]
fn clarity_adds_only_nine_contexts_and_eighteen_authors_no_forms_or_readings() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 946);
    assert_eq!(en.sentences().count(), 1856);
    let layer = &en.word_layers()[62];
    assert_eq!(layer.id, "daily_essentials_dialogue");
    assert!(layer.words.is_empty());
    // Only this batch's fixed window is historical; later tails are not its data.
    assert!(layer.next_words.len() >= 15);
    assert!(layer.sentences.len() >= 30);
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[12 + i].0, context);
        assert_eq!(layer.next_words[12 + i].1, words);
        assert_eq!(&layer.sentences[24 + 2 * i..26 + 2 * i], authors);
    }
    for (language, entries, reads, contexts, authors, start) in [
        ("zh-Hans", ZH, 3442, 937, 1872, 900),
        ("ja", JA, 638, 425, 849, 388),
    ] {
        let data = builtin(language).unwrap();
        assert_eq!(data.readings().len(), reads);
        assert_eq!(data.continuations().count(), contexts);
        assert_eq!(
            data.continuations().map(|(_, vs)| vs.len()).sum::<usize>(),
            authors
        );
        let tail: Vec<_> = data.continuations().skip(start).take(3).collect();
        assert_eq!(tail.len(), 3);
        for (i, &(_, _, word, pair)) in entries.iter().enumerate() {
            assert_eq!(tail[i].0, word);
            assert_eq!(tail[i].1, pair);
            assert!(data.readings().iter().any(|r| r.text == word));
        }
    }
    let forms: BTreeSet<_> = en
        .word_layers()
        .iter()
        .flat_map(|layer| {
            layer
                .words
                .iter()
                .map(String::as_str)
                .chain(
                    layer
                        .next_words
                        .iter()
                        .flat_map(|(_, ws)| ws.iter().map(String::as_str)),
                )
                .chain(layer.sentences.iter().flat_map(|s| {
                    s.split(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '\'' | '’')))
                        .filter(|s| !s.is_empty())
                }))
        })
        .map(|w| w.replace('’', "'").to_ascii_lowercase())
        .collect();
    assert_eq!(forms.len(), 6216);
    assert_eq!(
        sha_text(&forms.iter().map(|w| format!("{w}\n")).collect::<String>()),
        "23a7d72a05e5545016d5ad1f61e473d17491b95423f7f5b7ef0f677461b2327e"
    );
    for &(_, words, _, authors) in EN {
        assert!(words.iter().all(|w| is_known_english_word(w)));
        assert!(
            authors
                .iter()
                .flat_map(|s| s.split(|c: char| !(c.is_ascii_alphabetic() || c == '\'')))
                .filter(|w| !w.is_empty())
                .all(|w| forms.contains(w))
        );
    }
}

#[test]
fn clarity_english_full_partials_and_six_branches_commit_once() {
    for &(context, words, partials, authors) in EN {
        for i in 0..2 {
            let word = format!("{context} {}", words[i]);
            for seed in [
                format!("{context} "),
                format!("{context} {}", partials[i]),
                word.clone(),
            ] {
                SUITE.choices("en", &seed, &word, &[authors[i]]);
                for text in [&*word, authors[i], &*seed] {
                    SUITE.commit("en", &seed, text);
                }
            }
        }
    }
    // Contextual completion must not force an incomplete bare token to commit.
    for raw in ["wha", "wher"] {
        SUITE.commit("en", raw, raw);
    }
}

#[test]
fn clarity_chinese_joined_separated_case_tones_and_partials_offer_both_authors() {
    for &(reading, partial, word, authors) in ZH {
        for seed in [
            reading.into(),
            reading.replace('\'', ""),
            reading.replace('\'', " ").to_ascii_uppercase(),
            partial.into(),
        ] {
            SUITE.choices("zh-Hans", &seed, word, &authors);
            for text in [word, authors[0], authors[1], &seed] {
                SUITE.commit("zh-Hans", &seed, text);
            }
        }
    }
    for (i, toned) in ["fen1'qi2", "bu4'zhou4", "yi2'wen4"]
        .into_iter()
        .enumerate()
    {
        SUITE.choices("zh-Hans", toned, ZH[i].2, &ZH[i].3);
    }
}

#[test]
fn clarity_japanese_fixed_kana_romaji_case_and_partials_keep_terminal_n_raw() {
    for (raw, i) in [("tejunn", 1), ("mikakuninn", 2)] {
        SUITE.choices("ja", raw, JA[i].2, &JA[i].3);
        SUITE.commit("ja", raw, raw);
    }
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["ひつよう", "てじゅん", "みかくにん"][i];
        assert_eq!(romaji_to_hiragana(reading), kana);
        for seed in [
            reading.into(),
            reading.to_ascii_uppercase(),
            kana.into(),
            partial.into(),
            word.into(),
        ] {
            SUITE.choices("ja", &seed, word, &authors);
            for text in [word, authors[0], authors[1], &seed] {
                SUITE.commit("ja", &seed, text);
            }
        }
    }
    for (raw, kana) in [
        ("hitsuyo", "ひつよ"),
        ("teju", "てじゅ"),
        ("tejun", "てじゅん"),
        ("mikakuni", "みかくに"),
        ("mikakunin", "みかくにん"),
    ] {
        assert_eq!(romaji_to_hiragana(raw), kana);
        assert_eq!(SUITE.engine("ja", raw).snapshot().seed_text, raw);
        SUITE.commit("ja", raw, raw);
    }
}

#[test]
fn clarity_six_horizontal_gaps_adoption_and_long_drafts_keep_exact_prefixes() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = format!("before{gap}we{gap}start,{gap}i{gap}want{gap}to{gap}confirm{gap}wha");
        let word = seed.strip_suffix("wha").unwrap().to_owned() + "what";
        let author = word.clone() + " you need.";
        SUITE.choices("en", &seed, &word, &[&author]);
        for &(reading, _, word, authors) in ZH {
            let seed = reading.replace('\'', gap);
            SUITE.choices("zh-Hans", &seed, word, &authors);
            SUITE.commit("zh-Hans", &seed, &seed);
        }
        for (i, (left, right)) in [("hitsu", "you"), ("te", "jun"), ("mi", "kakunin")]
            .into_iter()
            .enumerate()
        {
            let seed = format!("{left}{gap}{right}");
            SUITE.choices("ja", &seed, JA[i].2, &JA[i].3);
            SUITE.commit("ja", &seed, &seed);
        }
    }
    for &(reading, _, word, authors) in ZH {
        SUITE.adopt("zh-Hans", reading, word, &authors);
    }
    for &(reading, _, word, authors) in JA {
        SUITE.adopt("ja", reading, word, &authors);
    }
    let prefix = "之前的记录🙂，不改写已有内容。".repeat(30);
    let seed = format!("{prefix}before we start, i want to confirm wha");
    let word = format!("{prefix}before we start, i want to confirm what");
    let author = format!("{word} you need.");
    SUITE.choices("en", &seed, &word, &[&author]);
    SUITE.commit("en", &seed, &author);
}

#[test]
fn clarity_raw_rescue_does_not_join_seven_line_boundaries_or_opaque_tokens() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, author) in [
            (
                "en",
                format!("before we start,{line}i want to confirm wha"),
                EN[0].3[0],
            ),
            ("zh-Hans", format!("fen{line}qi"), ZH[0].3[0]),
            ("ja", format!("hitsu{line}you"), JA[0].3[0]),
        ] {
            assert!(
                !SUITE
                    .engine(language, &seed)
                    .candidates()
                    .iter()
                    .any(|c| c.text == author)
            );
            SUITE.commit(language, &seed, &seed);
        }
    }
    for (language, seed) in [
        ("en", "https://example.invalid/confirm/wha"),
        ("en", "confirm_wha_42"),
        ("zh-Hans", "user_fen_qi_42"),
        ("ja", "user_hitsuyou_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
}

#[test]
fn clarity_pending_disabled_five_typed_failures_and_empty_models_keep_local_pools() {
    SUITE.missing_models(
        "en",
        "before we start, i want to confirm wha",
        "before we start, i want to confirm what",
        &EN[0].3[..1],
    );
    SUITE.missing_models("zh-Hans", "fen'q", ZH[0].2, &ZH[0].3);
    SUITE.missing_models("ja", "hitsuyo", JA[0].2, &JA[0].3);
}

fn old_controls(language: &str) {
    match language {
        "en" => SUITE.choices(
            "en",
            "give me a moment to finis",
            "give me a moment to finish",
            &["give me a moment to finish this message before we leave."],
        ),
        "zh-Hans" => SUITE.choices(
            "zh-Hans",
            "peng'to",
            "碰头",
            &[
                "碰头以后，我们先把今天的安排说清楚。",
                "碰头的时间我还在确认，稍后告诉你。",
            ],
        ),
        "ja" => SUITE.choices(
            "ja",
            "yoteihenko",
            "予定変更",
            &[
                "予定変更の件は、あとで一緒に確認しましょう。",
                "予定変更があれば、出発する前に教えてください。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn clarity_preserves_previous_coordination_author_controls() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn clarity_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs() {
    const NAME: &str = "clarity_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_CLARITY_DIALOGUE_TEST_CHILD",
        old_controls,
        |language| match language {
            "en" => {
                for &(context, words, partials, authors) in EN {
                    for i in 0..2 {
                        let seed = format!("{context} {}", partials[i]);
                        let word = format!("{context} {}", words[i]);
                        SUITE.choices("en", &seed, &word, &[authors[i]]);
                        SUITE.commit("en", &seed, authors[i]);
                        SUITE.commit("en", &seed, &seed);
                    }
                }
            }
            "zh-Hans" => {
                for &(reading, partial, word, authors) in ZH {
                    SUITE.choices("zh-Hans", reading, word, &authors);
                    let ime = SUITE.engine("zh-Hans", partial);
                    for author in authors {
                        assert!(
                            ime.candidates()
                                .iter()
                                .any(|c| c.text == author && c.kind == CandidateKind::Sentence)
                        );
                        SUITE.commit("zh-Hans", partial, author);
                    }
                    SUITE.commit("zh-Hans", partial, word);
                    SUITE.commit("zh-Hans", partial, partial);
                }
            }
            "ja" => {
                for &(_, partial, word, authors) in JA {
                    SUITE.choices("ja", partial, word, &authors);
                    for author in authors {
                        SUITE.commit("ja", partial, author);
                    }
                    SUITE.commit("ja", partial, partial);
                }
            }
            _ => unreachable!(),
        },
    );
}

#[test]
fn clarity_retains_all_six_preexisting_before_authors_without_promising_new_short_root_promotion() {
    let old = [
        "before leaving i'll adjust my belt so it fits better.",
        "before leaving i'll adjust my scarf to keep it in place.",
        "before closing this book i'll finish the page i'm reading.",
        "before closing this book i'll finish the chapter and leave a bookmark.",
        "before watering these plants i'll check the soil in each pot.",
        "before watering these plants i'll move my notebook away from the water.",
    ];
    let ime = SUITE.engine("en", "before");
    for author in old {
        assert!(
            ime.candidates()
                .iter()
                .any(|c| c.text == author && c.kind == CandidateKind::Sentence),
            "missing old author {author:?}: {:?}",
            ime.candidates()
        );
        SUITE.commit("en", "before", author);
    }
}

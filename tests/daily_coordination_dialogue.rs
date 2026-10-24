//! Authored coordination language, not messaging, scheduling or location services.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
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

const SUITE: Suite = Suite {
    marker: "COORDINATION",
};
// Frozen independently of the production resource and candidate output.
const EN: &[EnglishCase] = &[
    (
        "give me a moment to",
        ["finish", "check"],
        ["finis", "chec"],
        [
            "give me a moment to finish this message before we leave.",
            "give me a moment to check the address before we go.",
        ],
    ),
    (
        "we can talk again",
        ["later", "tomorrow"],
        ["late", "tomorro"],
        [
            "we can talk again later when you have time.",
            "we can talk again tomorrow after lunch.",
        ],
    ),
    (
        "tell me when you are",
        ["ready", "free"],
        ["read", "fre"],
        [
            "tell me when you are ready to leave.",
            "tell me when you are free for a quick call.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "peng'tou",
        "peng'to",
        "碰头",
        [
            "碰头以后，我们先把今天的安排说清楚。",
            "碰头的时间我还在确认，稍后告诉你。",
        ],
    ),
    (
        "kong'xian",
        "kong'xia",
        "空闲",
        [
            "空闲的时候我们再确认一下碰头时间。",
            "空闲时给我回个消息就好，不用着急。",
        ],
    ),
    (
        "shao'hou'lian'xi",
        "shao'hou'lian'x",
        "稍后联系",
        [
            "稍后联系你，先把手头的事情忙完。",
            "稍后联系也可以，我这边不着急。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "yoteihenkou",
        "yoteihenko",
        "予定変更",
        [
            "予定変更の件は、あとで一緒に確認しましょう。",
            "予定変更があれば、出発する前に教えてください。",
        ],
    ),
    (
        "touchakuyotei",
        "touchakuyote",
        "到着予定",
        [
            "到着予定は午後三時ごろです。",
            "到着予定が分かったら、短いメッセージを送ります。",
        ],
    ),
    (
        "buntan",
        "bunta",
        "分担",
        [
            "分担は、始める前に一緒に決めましょう。",
            "分担を変えたいときは、遠慮なく言ってください。",
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
fn coordination_preserves_exact_previous_resource_bytes_flags_order_and_authors() {
    let mut en = include_str!("../data/lexicons/en.json").to_owned();
    let layer_first = "\n    {\n      \"id\": \"daily_essentials_dialogue\",";
    let layer_after = "\n    }\n  ],\n  \"phrase_endings\": [";
    assert_eq!(en.matches(layer_first).count(), 1);
    let start = en.find(layer_first).unwrap();
    assert_eq!(en[start..].matches(layer_after).count(), 1);
    let end = start + en[start..].find(layer_after).unwrap();
    let mut layer = en[start..end].to_owned();
    for (first, after) in [
        (
            ",\n        [\n          \"give me a moment to\",",
            "\n      ],\n      \"sentences\": [",
        ),
        (
            ",\n        \"give me a moment to finish this message before we leave.\"",
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
        "7b83d83199a921f28b25a227ad150e98ef6e3ee8f60c5a2d5a0f084e7a39b44b"
    );
    for (raw, first, hash) in [
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            ",\n    [\n      \"碰头\",",
            "ed2e998006487fccdf70bcefe75e8980f26d2c21a4260e6da6676b887b3fe733",
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            ",\n    [\n      \"予定変更\",",
            "537be7069924d76339c1f4ed64b09450510a6d57f5249bea1077654b5394962d",
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
fn coordination_adds_only_nine_contexts_and_eighteen_authors_no_forms_or_readings() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 63);
    assert_eq!(en.next_words().count(), 915);
    assert_eq!(en.sentences().count(), 1794);
    let layer = &en.word_layers()[62];
    assert_eq!(layer.id, "daily_essentials_dialogue");
    assert!(layer.words.is_empty());
    assert!(layer.next_words.len() >= 12);
    assert!(layer.sentences.len() >= 24);
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[9 + i].0, context);
        assert_eq!(layer.next_words[9 + i].1, words);
        assert_eq!(&layer.sentences[18 + 2 * i..20 + 2 * i], authors);
    }
    for (language, entries, reads, contexts, authors, start) in [
        ("zh-Hans", ZH, 3442, 906, 1810, 897),
        ("ja", JA, 638, 394, 787, 385),
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
fn coordination_english_full_partials_and_six_branches_commit_once() {
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
    // `late` and `read` remain valid raw words; these are contextual branches only.
    for raw in ["late", "read"] {
        SUITE.commit("en", raw, raw);
    }
}

#[test]
fn coordination_chinese_joined_separated_case_tones_and_partials_offer_both_authors() {
    // This short prefix must retain the old storage Word alongside new authors.
    SUITE.choices("zh-Hans", "kong'xia", "空箱子", &[]);
    SUITE.commit("zh-Hans", "kong'xia", "空箱子");
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
    for (i, toned) in ["peng4'tou2", "kong4'xian2", "shao1'hou4'lian2'xi4"]
        .into_iter()
        .enumerate()
    {
        SUITE.choices("zh-Hans", toned, ZH[i].2, &ZH[i].3);
    }
}

#[test]
fn coordination_japanese_fixed_kana_romaji_case_and_partials_keep_terminal_n_raw() {
    SUITE.choices("ja", "buntann", JA[2].2, &JA[2].3);
    SUITE.commit("ja", "buntann", "buntann");
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["よていへんこう", "とうちゃくよてい", "ぶんたん"][i];
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
        ("yoteihenko", "よていへんこ"),
        ("touchakuyote", "とうちゃくよて"),
        ("bunta", "ぶんた"),
        ("buntan", "ぶんたん"),
    ] {
        assert_eq!(romaji_to_hiragana(raw), kana);
        assert_eq!(SUITE.engine("ja", raw).snapshot().seed_text, raw);
        SUITE.commit("ja", raw, raw);
    }
}

#[test]
fn coordination_six_horizontal_gaps_adoption_and_long_drafts_keep_exact_prefixes() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = format!("give{gap}me{gap}a{gap}moment{gap}to{gap}finis");
        let word = seed.strip_suffix("finis").unwrap().to_owned() + "finish";
        let author = word.clone() + " this message before we leave.";
        SUITE.choices("en", &seed, &word, &[&author]);
        for &(reading, _, word, authors) in ZH {
            let seed = reading.replace('\'', gap);
            SUITE.choices("zh-Hans", &seed, word, &authors);
            SUITE.commit("zh-Hans", &seed, &seed);
        }
        for (i, (left, right)) in [("yotei", "henkou"), ("touchaku", "yotei"), ("bun", "tan")]
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
    let seed = format!("{prefix}give me a moment to finis");
    let word = format!("{prefix}give me a moment to finish");
    let author = format!("{word} this message before we leave.");
    SUITE.choices("en", &seed, &word, &[&author]);
    SUITE.commit("en", &seed, &author);
}

#[test]
fn coordination_raw_rescue_does_not_join_seven_line_boundaries_or_opaque_tokens() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, author) in [
            ("en", format!("give me a moment{line}to finis"), EN[0].3[0]),
            ("zh-Hans", format!("peng{line}tou"), ZH[0].3[0]),
            ("ja", format!("yotei{line}henkou"), JA[0].3[0]),
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
        ("en", "https://example.invalid/give_me/finis"),
        ("en", "moment_finis_42"),
        ("zh-Hans", "user_peng_tou_42"),
        ("ja", "user_yoteihenkou_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
}

#[test]
fn coordination_pending_disabled_five_typed_failures_and_empty_models_keep_local_pools() {
    SUITE.missing_models(
        "en",
        "give me a moment to finis",
        "give me a moment to finish",
        &EN[0].3[..1],
    );
    SUITE.missing_models("zh-Hans", "peng'to", ZH[0].2, &ZH[0].3);
    SUITE.missing_models("ja", "yoteihenko", JA[0].2, &JA[0].3);
}

fn old_controls(language: &str) {
    match language {
        "en" => SUITE.choices(
            "en",
            "the comb is still in the drawe",
            "the comb is still in the drawer",
            &["the comb is still in the drawer with my hairbrush."],
        ),
        "zh-Hans" => SUITE.choices(
            "zh-Hans",
            "yu'sa",
            "雨伞",
            &["雨伞放在玄关的伞架里。", "雨伞在门边，和折叠伞放在一起。"],
        ),
        "ja" => SUITE.choices(
            "ja",
            "reinko-t",
            "レインコート",
            &[
                "レインコートは玄関の棚の下にあります。",
                "レインコートを入れた袋は鞄の横にあります。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn coordination_preserves_previous_small_essentials_author_controls() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn coordination_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs() {
    const NAME: &str =
        "coordination_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_COORDINATION_DIALOGUE_TEST_CHILD",
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

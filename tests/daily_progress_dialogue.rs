//! Authored progress replies, not state sensing or task execution.
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

const SUITE: Suite = Suite { marker: "PROGRESS" };
// Frozen independently of the production resource and candidate output.
const EN: &[EnglishCase] = &[
    (
        "the first part is",
        ["done", "ready"],
        ["don", "read"],
        [
            "the first part is done, and i'm checking the rest.",
            "the first part is ready, but the last part still needs work.",
        ],
    ),
    (
        "one detail is still",
        ["missing", "wrong"],
        ["missin", "wron"],
        [
            "one detail is still missing, so i can't finish this part yet.",
            "one detail is still wrong, so i'll fix it before we continue.",
        ],
    ),
    (
        "once this part is done, we can",
        ["review", "start"],
        ["revie", "star"],
        [
            "once this part is done, we can review it together.",
            "once this part is done, we can start the next one.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "wei'wan'cheng",
        "wei'wan'chen",
        "未完成",
        [
            "未完成的部分我先列出来，等信息齐了再继续。",
            "未完成的地方还需要确认，今天先到这里吧。",
        ],
    ),
    (
        "hai'zai'he'dui",
        "hai'zai'he'du",
        "还在核对",
        [
            "还在核对最后一处，确认后马上告诉你。",
            "还在核对，暂时先不要按这个版本继续。",
        ],
    ),
    (
        "hou'xu'an'pai",
        "hou'xu'an'pa",
        "后续安排",
        [
            "后续安排可以先列出来，我们再一起选。",
            "后续安排我还想再确认一下，晚点回复你。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "owarimashita",
        "owarimashit",
        "終わりました",
        [
            "終わりました。確認してほしいところだけ、あとでまとめて伝えます。",
            "終わりましたので、次に進む前に一度見てもらえますか。",
        ],
    ),
    (
        "chien",
        "chie",
        "遅延",
        [
            "遅延の理由を確認してから、次の予定を決めましょう。",
            "遅延が分かった時点で、ひと言知らせてもらえると助かります。",
        ],
    ),
    (
        "misoushin",
        "misoushi",
        "未送信",
        [
            "未送信の内容を見直してから、もう一度確認します。",
            "未送信のまま残っているので、送る前に相談します。",
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
fn progress_preserves_exact_previous_resource_bytes_flags_order_and_authors() {
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
            ",\n        [\n          \"the first part is\",",
            "\n      ],\n      \"sentences\": [",
        ),
        (
            ",\n        \"the first part is done, and i'm checking the rest.\"",
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
        "caba7d31753c4b472e404c40a3e0b82232f157c845bcdef69bc64f54982681c5"
    );
    for (raw, first, hash) in [
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            ",\n    [\n      \"卡住\",",
            "2f2953249aba6680b4bb4b47da6a512a05eda177aa37d77234f541e052a27f22",
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            ",\n    [\n      \"終わりました\",",
            "df0e779bc24260a6bef3fcc39280b1d547ba1091cae93043918360a4fe0f1abe",
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
fn progress_adds_only_nine_contexts_and_eighteen_authors_no_forms_or_readings() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 930);
    assert_eq!(en.sentences().count(), 1824);
    let layer = &en.word_layers()[62];
    assert_eq!(layer.id, "daily_essentials_dialogue");
    assert!(layer.words.is_empty());
    // Only this batch's fixed window is historical; later tails are not its data.
    assert!(layer.next_words.len() >= 18);
    assert!(layer.sentences.len() >= 36);
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[15 + i].0, context);
        assert_eq!(layer.next_words[15 + i].1, words);
        assert_eq!(&layer.sentences[30 + 2 * i..32 + 2 * i], authors);
    }
    for (language, entries, reads, contexts, authors, start) in [
        ("zh-Hans", ZH, 3442, 921, 1840, 903),
        ("ja", JA, 638, 409, 817, 391),
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
fn progress_english_full_partials_and_six_branches_commit_once() {
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
    for raw in ["read", "star"] {
        SUITE.commit("en", raw, raw);
    }
}

#[test]
fn progress_chinese_joined_separated_case_tones_and_partials_offer_both_authors() {
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
    for (i, toned) in [
        "wei4'wan2'cheng2",
        "hai2'zai4'he2'dui4",
        "hou4'xu4'an1'pai2",
    ]
    .into_iter()
    .enumerate()
    {
        SUITE.choices("zh-Hans", toned, ZH[i].2, &ZH[i].3);
    }
}

#[test]
fn progress_japanese_fixed_kana_romaji_case_and_partials_keep_terminal_n_raw() {
    for (raw, i) in [("chienn", 1), ("misoushinn", 2)] {
        SUITE.choices("ja", raw, JA[i].2, &JA[i].3);
        SUITE.commit("ja", raw, raw);
    }
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["おわりました", "ちえん", "みそうしん"][i];
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
        ("owarimashit", "おわりましt"),
        ("chie", "ちえ"),
        ("chien", "ちえん"),
        ("misoushi", "みそうし"),
        ("misoushin", "みそうしん"),
    ] {
        assert_eq!(romaji_to_hiragana(raw), kana);
        assert_eq!(SUITE.engine("ja", raw).snapshot().seed_text, raw);
        SUITE.commit("ja", raw, raw);
    }
}

#[test]
fn progress_six_horizontal_gaps_adoption_and_long_drafts_keep_exact_prefixes() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = format!("the{gap}first{gap}part{gap}is{gap}don");
        let word = seed.strip_suffix("don").unwrap().to_owned() + "done";
        let author = word.clone() + ", and i'm checking the rest.";
        SUITE.choices("en", &seed, &word, &[&author]);
        for &(reading, _, word, authors) in ZH {
            let seed = reading.replace('\'', gap);
            SUITE.choices("zh-Hans", &seed, word, &authors);
            SUITE.commit("zh-Hans", &seed, &seed);
        }
        for (i, (left, right)) in [("owari", "mashita"), ("chi", "en"), ("mi", "soushin")]
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
    let seed = format!("{prefix}the first part is don");
    let word = format!("{prefix}the first part is done");
    let author = format!("{word}, and i'm checking the rest.");
    SUITE.choices("en", &seed, &word, &[&author]);
    SUITE.commit("en", &seed, &author);
}

#[test]
fn progress_raw_rescue_does_not_join_seven_line_boundaries_or_opaque_tokens() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, author) in [
            ("en", format!("the first part{line}is don"), EN[0].3[0]),
            ("zh-Hans", format!("wei{line}wan cheng"), ZH[0].3[0]),
            ("ja", format!("owari{line}mashita"), JA[0].3[0]),
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
        ("en", "https://example.invalid/first_part/don"),
        ("en", "first_part_don_42"),
        ("zh-Hans", "user_wei_wan_cheng_42"),
        ("ja", "user_owarimashita_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
}

#[test]
fn progress_pending_disabled_five_typed_failures_and_empty_models_keep_local_pools() {
    SUITE.missing_models(
        "en",
        "the first part is don",
        "the first part is done",
        &EN[0].3[..1],
    );
    SUITE.missing_models("zh-Hans", "wei'wan'chen", ZH[0].2, &ZH[0].3);
    SUITE.missing_models("ja", "owarimashit", JA[0].2, &JA[0].3);
}

fn old_controls(language: &str) {
    match language {
        "en" => SUITE.choices(
            "en",
            "before we start, i want to confirm wha",
            "before we start, i want to confirm what",
            &["before we start, i want to confirm what you need."],
        ),
        "zh-Hans" => SUITE.choices(
            "zh-Hans",
            "fen'q",
            "分歧",
            &[
                "分歧主要在先做哪一步，我们再对一遍。",
                "分歧可以慢慢说清楚，先听听你的想法。",
            ],
        ),
        "ja" => SUITE.choices(
            "ja",
            "hitsuyo",
            "必要",
            &[
                "必要な条件を、先に二つだけ教えてもらえますか。",
                "必要なら、別の例で説明してもらえますか。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn progress_preserves_previous_clarity_author_controls() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn progress_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs() {
    const NAME: &str = "progress_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_PROGRESS_DIALOGUE_TEST_CHILD",
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
fn progress_retains_old_once_and_one_authors_in_the_full_pool_and_commits_them() {
    let once = [
        "once i've finished writing i'll put the pen back in the drawer.",
        "once i've finished writing i'll put the ruler next to my notebook.",
    ];
    let one = [
        "one of the items is missing.",
        "one of the items is damaged.",
        "one shoelace is caught under the shoe so i need a moment.",
        "one shoelace is caught under the bag i left by the door.",
    ];
    // No promise that the new long-context sentences appear for these bare roots.
    for (seed, authors) in [("once", &once[..]), ("one", &one[..])] {
        let ime = SUITE.engine("en", seed);
        for author in authors {
            assert!(
                ime.candidates()
                    .iter()
                    .any(|c| c.text == *author && c.kind == CandidateKind::Sentence),
                "missing old {author:?}: {:?}",
                ime.candidates()
            );
            SUITE.commit("en", seed, author);
        }
    }
}

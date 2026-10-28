//! Finite away/back conversation fallback, not presence detection or scheduling.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/presence_resource.rs"]
mod presence_resource;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::BTreeMap;
use suzaku_map::{
    ime::candidate_mix::CandidateKind, languages::japanese::romaji_to_hiragana, lexicon::builtin,
};

const SUITE: Suite = Suite { marker: "PRESENCE" };
const EN: &[EnglishCase] = &[
    (
        "i need to step away from the",
        ["chat", "screen"],
        ["cha", "scree"],
        [
            "i need to step away from the chat for a few minutes.",
            "i need to step away from the screen until after lunch.",
        ],
    ),
    (
        "i'm back and can",
        ["continue", "listen"],
        ["continu", "liste"],
        [
            "i'm back and can continue where we left off.",
            "i'm back and can listen to the rest now.",
        ],
    ),
    (
        "let's continue this after",
        ["lunch", "dinner"],
        ["lunc", "dinne"],
        [
            "let's continue this after lunch when we're both free.",
            "let's continue this after dinner if you have time.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "zan'ting'yi'xia",
        "zan'ting'yi'x",
        "暂停一下",
        [
            "暂停一下，我去拿点东西，马上回来。",
            "暂停一下，等我回来再接着说。",
        ],
    ),
    (
        "ji'dian'hui'lai",
        "ji'dian'hui'l",
        "几点回来",
        [
            "几点回来呢？我想等你一起吃饭。",
            "几点回来方便？我们到时候再继续聊。",
        ],
    ),
    (
        "shao'hou'tao'lun",
        "shao'hou'tao'l",
        "稍后讨论",
        [
            "稍后讨论也可以，我先把手头的事做完。",
            "稍后讨论这个问题，等大家都有空再继续。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "tadaima",
        "tadaim",
        "ただいま",
        [
            "ただいま。さっきの続きを一緒に確認してもいいですか。",
            "ただいま戻りました。途中で変わった点があれば教えてください。",
        ],
    ),
    (
        "fuzai",
        "fuza",
        "不在",
        [
            "不在の間の連絡は、戻ってからまとめて確認します。",
            "不在になる時間だけ、先に伝えておきます。",
        ],
    ),
    (
        "taishutsu",
        "taishut",
        "退室",
        [
            "退室する前に、戻る予定の時間だけお伝えします。",
            "退室しますが、戻ったらさっきの続きを一緒に確認しましょう。",
        ],
    ),
];

#[test]
fn presence_appends_exact_nine_contexts_eighteen_authors_and_keeps_published_resources() {
    for (raw, first, after, previous_sha) in [
        (
            include_str!("../data/lexicons/en.json"),
            ",\n    {\n      \"id\": \"daily_presence_dialogue\",",
            "\n  ],\n  \"phrase_endings\": [",
            "899d6e92d29729e7ea31d5d3e4cda420f9c712381caf4dded030a9a99e4a7bd3",
        ),
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            ",\n    [\n      \"暂停一下\",",
            "\n  ]\n}",
            "61734b2ea54c008bff68cd7ae7afb528ab57b29de493e555f63063c145ea6164",
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            ",\n    [\n      \"ただいま\",",
            "\n  ]\n}",
            "68019e118f4422174fe6f6ba063129dba1ba70f87dd9e00a9c16e0f25b7d5269",
        ),
    ] {
        presence_resource::without_presence(raw);
        // Exercise the shared exact-byte guard as well as the semantic inverse.
        let previous = presence_resource::without_turntaking(raw);
        SUITE.preserve_resource(&previous, first, after, previous_sha);
    }
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 968);
    assert_eq!(en.sentences().count(), 1900);
    let layer = &en.word_layers()[63];
    assert_eq!(layer.id, "daily_presence_dialogue");
    assert!(layer.words.is_empty());
    assert!(layer.next_words.len() >= 3);
    assert!(layer.sentences.len() >= 6);
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[i].0, context);
        assert_eq!(layer.next_words[i].1, words);
        assert_eq!(&layer.sentences[2 * i..2 * i + 2], authors);
    }
    for (language, entries, readings, start, contexts, authors) in [
        ("zh-Hans", ZH, 3442, 906, 959, 1916),
        ("ja", JA, 638, 394, 447, 893),
    ] {
        let data = builtin(language).unwrap();
        assert_eq!(data.readings().len(), readings);
        assert_eq!(data.continuations().count(), contexts);
        assert_eq!(
            data.continuations().map(|(_, a)| a.len()).sum::<usize>(),
            authors
        );
        for ((word, pair), &(_, _, expected, authors)) in
            data.continuations().skip(start).zip(entries)
        {
            assert_eq!(word, expected);
            assert_eq!(pair, authors);
        }
    }
}

#[test]
fn presence_keeps_all_6216_published_english_first_seen_ranks() {
    let mut ranks = BTreeMap::new();
    let mut rank = 0;
    for layer in builtin("en").unwrap().word_layers() {
        for word in layer
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
                    .filter(|w| !w.is_empty())
            }))
        {
            ranks
                .entry(word.replace('’', "'").to_ascii_lowercase())
                .or_insert(rank);
            rank += 1;
        }
    }
    assert_eq!(ranks.len(), 6216);
    let old = ranks
        .iter()
        .map(|(word, rank)| format!("{word}\0{rank}\n"))
        .collect::<String>();
    let hash = digest(&SHA256, old.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(
        hash,
        "edd8b14aa834036441481651ec3cd72d0a00e36c812756f5b292a726789fbd3a"
    );
}

#[test]
fn presence_english_six_branches_full_and_partials_commit_once() {
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
}

#[test]
fn presence_chinese_full_joined_case_partials_and_tones_offer_both_authors() {
    for (i, &(reading, partial, word, authors)) in ZH.iter().enumerate() {
        for seed in [
            reading.into(),
            reading.replace('\'', ""),
            reading.replace('\'', " ").to_ascii_uppercase(),
            partial.into(),
            word.into(),
            [
                "zan4'ting2'yi2'xia4",
                "ji3'dian3'hui2'lai2",
                "shao1'hou4'tao3'lun4",
            ][i]
                .into(),
        ] {
            SUITE.choices("zh-Hans", &seed, word, &authors);
            for text in [word, authors[0], authors[1], &seed] {
                SUITE.commit("zh-Hans", &seed, text);
            }
        }
    }
}

#[test]
fn presence_japanese_full_case_fixed_kana_and_partials_offer_both_authors() {
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["ただいま", "ふざい", "たいしゅつ"][i];
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
}

#[test]
fn presence_horizontal_gaps_adoption_and_long_english_drafts_preserve_prefixes() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = ["i'm", "back", "and", "can", "continu"].join(gap);
        let word = seed.strip_suffix("continu").unwrap().to_owned() + "continue";
        SUITE.choices(
            "en",
            &seed,
            &word,
            &[&(word.clone() + " where we left off.")],
        );
        for &(reading, _, word, authors) in ZH {
            SUITE.choices("zh-Hans", &reading.replace('\'', gap), word, &authors);
        }
        for (i, (a, b)) in [("tada", "ima"), ("fu", "zai"), ("tai", "shutsu")]
            .into_iter()
            .enumerate()
        {
            SUITE.choices("ja", &format!("{a}{gap}{b}"), JA[i].2, &JA[i].3);
        }
    }
    for (language, entries) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(reading, _, word, authors) in entries {
            SUITE.adopt(language, reading, word, &authors);
        }
    }
    let prefix = "前面的文字🙂，保持原样。".repeat(35);
    let seed = format!("{prefix}i'm back and can continu");
    let word = format!("{prefix}i'm back and can continue");
    let author = format!("{word} where we left off.");
    SUITE.choices("en", &seed, &word, &[&author]);
    SUITE.commit("en", &seed, &author);
}

#[test]
fn presence_raw_rescue_does_not_bridge_lines_or_opaque_tokens() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, author) in [
            ("en", format!("i'm back and{line}can continu"), EN[1].3[0]),
            ("zh-Hans", format!("zan{line}ting yi x"), ZH[0].3[0]),
            ("ja", format!("tai{line}shutsu"), JA[2].3[0]),
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
        ("en", "i'm back and can field_continu"),
        (
            "en",
            "https://example.invalid/i_need_to_step_away_from_the_cha",
        ),
        ("zh-Hans", "user_zan_ting_yi_x_42"),
        ("ja", "user_taishutsu_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
}

#[test]
fn presence_synthetic_pending_disabled_errors_and_empty_models_keep_local_candidates() {
    SUITE.missing_models(
        "en",
        "i'm back and can continu",
        "i'm back and can continue",
        &EN[1].3[..1],
    );
    SUITE.missing_models("zh-Hans", ZH[0].1, ZH[0].2, &ZH[0].3);
    SUITE.missing_models("ja", JA[1].1, JA[1].2, &JA[1].3);
}

fn old_controls(language: &str) {
    match language {
        "en" => SUITE.choices(
            "en",
            "the first part is don",
            "the first part is done",
            &["the first part is done, and i'm checking the rest."],
        ),
        "zh-Hans" => SUITE.choices(
            "zh-Hans",
            "wei'wan'chen",
            "未完成",
            &[
                "未完成的部分我先列出来，等信息齐了再继续。",
                "未完成的地方还需要确认，今天先到这里吧。",
            ],
        ),
        "ja" => SUITE.choices(
            "ja",
            "misoushi",
            "未送信",
            &[
                "未送信の内容を見直してから、もう一度確認します。",
                "未送信のまま残っているので、送る前に相談します。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn presence_previous_progress_authors_still_commit_once() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn presence_old_and_new_choices_coexist_with_private_8_8_9_packs() {
    const NAME: &str = "presence_old_and_new_choices_coexist_with_private_8_8_9_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_PRESENCE_DIALOGUE_TEST_CHILD",
        old_controls,
        |language| {
            if language == "en" {
                for &(context, words, partials, authors) in EN {
                    for i in 0..2 {
                        let seed = format!("{context} {}", partials[i]);
                        let word = format!("{context} {}", words[i]);
                        SUITE.choices(language, &seed, &word, &[authors[i]]);
                        SUITE.commit(language, &seed, authors[i]);
                    }
                }
            } else {
                for &(reading, partial, word, authors) in if language == "ja" { JA } else { ZH } {
                    for seed in [reading, partial] {
                        SUITE.choices(language, seed, word, &authors);
                        for text in [authors[0], authors[1], word, seed] {
                            SUITE.commit(language, seed, text);
                        }
                    }
                }
            }
        },
    );
}

#[test]
fn presence_raw_kana_and_terminal_n_controls_are_not_rewritten() {
    for (raw, kana) in [
        ("tadaim", "ただいm"),
        ("fuza", "ふざ"),
        ("taishut", "たいしゅt"),
        ("chien", "ちえん"),
        ("chienn", "ちえん"),
        ("misoushin", "みそうしん"),
        ("misoushinn", "みそうしん"),
    ] {
        assert_eq!(romaji_to_hiragana(raw), kana);
        SUITE.commit("ja", raw, raw);
    }
}

#[test]
fn presence_resource_inverse_rejects_changed_old_bytes_and_new_authors() {
    let raw = include_str!("../data/lexicons/en.json");
    let changed = raw.replacen("very much", "VERY MUCH", 1);
    assert_ne!(changed, raw);
    assert!(std::panic::catch_unwind(|| presence_resource::without_presence(&changed)).is_err());
    let raw = include_str!("../data/lexicons/ja.json");
    if raw.contains(JA[0].3[0]) {
        let changed = raw.replacen(JA[0].3[0], "ただいま。変更した作者です。", 1);
        assert!(
            std::panic::catch_unwind(|| presence_resource::without_presence(&changed)).is_err()
        );
    }
    // An unchanged converter/literal candidate is not a fabricated dictionary Word.
    let ime = SUITE.engine("ja", "ただいま");
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == "ただいま" && c.kind == CandidateKind::Literal)
    );
}

//! Authored everyday-item vocabulary, not item sensing or tracking.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/presence_resource.rs"]
mod presence_resource;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::BTreeMap;
use suzaku_map::{
    ime::candidate_mix::CandidateKind,
    languages::{english::is_known_english_word, japanese::romaji_to_hiragana},
    lexicon::builtin,
};

const SUITE: Suite = Suite {
    marker: "SMALL_ESSENTIALS",
};
// Independently frozen literals, not constructed from the production resource.
const EN: &[EnglishCase] = &[
    (
        "the comb is still in the",
        ["drawer", "pouch"],
        ["drawe", "pouc"],
        [
            "the comb is still in the drawer with my hairbrush.",
            "the comb is still in the pouch with my handkerchief.",
        ],
    ),
    (
        "my handkerchief is still in the",
        ["pocket", "drawer"],
        ["pocke", "drawe"],
        [
            "my handkerchief is still in the pocket of my jacket.",
            "my handkerchief is still in the drawer with my scarf.",
        ],
    ),
    (
        "i left the hairbrush on the",
        ["shelf", "table"],
        ["shel", "tabl"],
        [
            "i left the hairbrush on the shelf next to the shampoo.",
            "i left the hairbrush on the table by the bed.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "yu'san",
        "yu'sa",
        "雨伞",
        ["雨伞放在玄关的伞架里。", "雨伞在门边，和折叠伞放在一起。"],
    ),
    (
        "yao'shi'chuan",
        "yao'shi'chua",
        "钥匙串",
        [
            "钥匙串放在门边的小盒里。",
            "钥匙串在收纳袋的侧袋里，我帮你拿过来。",
        ],
    ),
    (
        "er'sai",
        "er'sa",
        "耳塞",
        ["耳塞放在随身的小袋里。", "耳塞在抽屉前面，我帮你拿过来。"],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "higasa",
        "higas",
        "日傘",
        [
            "日傘は玄関の傘立てにあります。",
            "日傘を入れた袋は靴箱の横にあります。",
        ],
    ),
    (
        "kagitaba",
        "kagitab",
        "鍵束",
        [
            "鍵束は玄関の小さな皿に置いてあります。",
            "鍵束を鞄の内ポケットに入れてあります。",
        ],
    ),
    (
        "reinko-to",
        "reinko-t",
        "レインコート",
        [
            "レインコートは玄関の棚の下にあります。",
            "レインコートを入れた袋は鞄の横にあります。",
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
fn small_essentials_preserves_exact_old_resources_flags_order_and_authors() {
    let mut en = presence_resource::without_presence(include_str!("../data/lexicons/en.json"));
    for (first, after) in [
        (
            ",\n        [\n          \"the comb is still in the\",",
            "\n      ],\n      \"sentences\": [",
        ),
        (
            ",\n        \"the comb is still in the drawer with my hairbrush.\"",
            "\n      ]\n    }\n  ],\n  \"phrase_endings\": [",
        ),
    ] {
        if en.contains(first) {
            assert_eq!(en.matches(first).count(), 1);
            let start = en.find(first).unwrap();
            assert!(start > en.find("\"id\": \"daily_essentials_dialogue\"").unwrap());
            let end = start + en[start..].find(after).unwrap();
            assert!(start < end);
            en.replace_range(start..end, "");
        }
    }
    assert_eq!(
        sha_text(&en),
        "9ef6f80aedd0e1ffea70bc183abdb0d0bb551fc6808a80c1f92681bd141c3cb4"
    );
    for (raw, first, hash) in [
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            ",\n    [\n      \"雨伞\",",
            "dcc3f0a1c68ecea2e6c4c2e300611eecc3d89bef7e86e97552ce11f0737ca53d",
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            ",\n    [\n      \"日傘\",",
            "6b0e58f650657267c1bd5670a852fb1e23f1a6b93347c979ab80dcf600ea0282",
        ),
    ] {
        SUITE.preserve_resource(raw, first, "\n  ]\n}", hash);
    }
}

#[test]
fn small_essentials_adds_nine_contexts_eighteen_authors_and_only_three_english_forms() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 930);
    assert_eq!(en.sentences().count(), 1824);
    let layer = &en.word_layers()[62];
    assert_eq!(layer.id, "daily_essentials_dialogue");
    assert!(layer.words.is_empty());
    assert!(layer.next_words.len() >= 9 && layer.sentences.len() >= 18);
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[6 + i].0, context);
        assert_eq!(layer.next_words[6 + i].1, words);
        assert_eq!(&layer.sentences[12 + 2 * i..14 + 2 * i], authors);
    }
    for (language, entries, reads, contexts, authors, start) in [
        ("zh-Hans", ZH, 3442, 921, 1840, 894),
        ("ja", JA, 638, 409, 817, 382),
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
}

#[test]
fn small_essentials_new_forms_complete_without_reordering_the_6213_old_first_seen_ranks() {
    let mut ranks = BTreeMap::new();
    let mut rank = 0;
    for layer in builtin("en").unwrap().word_layers() {
        for text in layer
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
        {
            ranks
                .entry(text.replace('’', "'").to_ascii_lowercase())
                .or_insert(rank);
            rank += 1;
        }
    }
    assert_eq!(ranks.len(), 6216);
    for word in ["comb", "handkerchief", "hairbrush"] {
        assert!(is_known_english_word(word));
        assert!(ranks.remove(word).is_some());
        SUITE.commit("en", word, word);
    }
    assert_eq!(ranks.len(), 6213);
    let old = ranks
        .iter()
        .map(|(word, rank)| format!("{word}\0{rank}\n"))
        .collect::<String>();
    assert_eq!(
        sha_text(&old),
        "1de0362ce9facd60cbf612f4565e0351bb13f207a6cae39999081dd39f59fc83"
    );
    for (partial, word) in [("handkerchie", "handkerchief"), ("hairbrus", "hairbrush")] {
        SUITE.choices("en", partial, word, &[]);
        SUITE.commit("en", partial, word);
    }
    // Ambiguous bare `com` is not promised to promote a new late-ranked word.
    SUITE.commit("en", "com", "com");
}

#[test]
fn small_essentials_english_full_partials_and_all_six_branches_commit_exactly_once() {
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
fn small_essentials_chinese_joined_separated_case_tones_and_partials_offer_both_authors() {
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
    for (i, toned) in ["yu3'san3", "yao4'shi5'chuan4", "er3'sai1"]
        .into_iter()
        .enumerate()
    {
        SUITE.choices("zh-Hans", toned, ZH[i].2, &ZH[i].3);
    }
}

#[test]
fn small_essentials_japanese_fixed_kana_romaji_case_and_partials_preserve_raw_consonants() {
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["ひがさ", "かぎたば", "れいんこーと"][i];
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
        ("higas", "ひがs"),
        ("kagitab", "かぎたb"),
        ("reinko-t", "れいんこーt"),
    ] {
        assert_eq!(romaji_to_hiragana(raw), kana);
        assert_eq!(SUITE.engine("ja", raw).snapshot().seed_text, raw);
        SUITE.commit("ja", raw, raw);
    }
}

#[test]
fn small_essentials_six_horizontal_spaces_adoption_and_long_drafts_keep_exact_prefixes() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = format!("the{gap}comb{gap}is{gap}still{gap}in{gap}the{gap}drawe");
        let word = seed.strip_suffix("drawe").unwrap().to_owned() + "drawer";
        let author = word.clone() + " with my hairbrush.";
        SUITE.choices("en", &seed, &word, &[&author]);
        for (i, &(reading, _, _, _)) in ZH.iter().enumerate() {
            let seed = reading.replace('\'', gap);
            SUITE.choices("zh-Hans", &seed, ZH[i].2, &ZH[i].3);
            SUITE.commit("zh-Hans", &seed, &seed);
        }
        for (i, (left, right)) in [("hi", "gasa"), ("kagi", "taba"), ("rein", "ko-to")]
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
    let seed = format!("{prefix}the comb is still in the drawe");
    let word = format!("{prefix}the comb is still in the drawer");
    let author = format!("{word} with my hairbrush.");
    SUITE.choices("en", &seed, &word, &[&author]);
    SUITE.commit("en", &seed, &author);
}

#[test]
fn small_essentials_raw_rescue_does_not_join_seven_line_boundaries_or_opaque_tokens() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, author) in [
            (
                "en",
                format!("the comb is still{line}in the drawe"),
                EN[0].3[0],
            ),
            ("zh-Hans", format!("yu{line}san"), ZH[0].3[0]),
            ("ja", format!("hi{line}gasa"), JA[0].3[0]),
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
        ("en", "https://example.invalid/the_comb/drawe"),
        ("en", "comb_drawe_42"),
        ("zh-Hans", "user_yu_san_42"),
        ("ja", "user_higasa_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
}

#[test]
fn small_essentials_pending_disabled_five_typed_failures_and_empty_models_keep_full_local_pool() {
    SUITE.missing_models(
        "en",
        "the comb is still in the drawe",
        "the comb is still in the drawer",
        &EN[0].3[..1],
    );
    SUITE.missing_models("zh-Hans", "yu'sa", ZH[0].2, &ZH[0].3);
    SUITE.missing_models("ja", "higas", JA[0].2, &JA[0].3);
}

fn old_controls(language: &str) {
    match language {
        "en" => SUITE.choices(
            "en",
            "my scarf is still on the chai",
            "my scarf is still on the chair",
            &["my scarf is still on the chair by the door."],
        ),
        "zh-Hans" => SUITE.choices(
            "zh-Hans",
            "shu'z",
            "梳子",
            &["梳子放在洗漱包的小袋里。", "梳子在镜子旁边，我帮你拿过来。"],
        ),
        "ja" => SUITE.choices(
            "ja",
            "tebukur",
            "手袋",
            &[
                "手袋は上着のポケットに入っています。",
                "手袋を入れた袋は玄関の棚にあります。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn small_essentials_preserves_previous_personal_item_author_controls() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn small_essentials_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs() {
    const NAME: &str =
        "small_essentials_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_SMALL_ESSENTIALS_TEST_CHILD",
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

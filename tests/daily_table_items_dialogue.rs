//! Fixed table-item location dialogue; no sensing, food advice or device control.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/living_room_reading.rs"]
mod living_room_reading;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use std::collections::HashSet;
use suzaku_map::{
    ime::candidate_mix::CandidateKind, languages::japanese::romaji_to_hiragana, lexicon::builtin,
};

const SUITE: Suite = Suite {
    marker: "TABLE_ITEMS",
};
// Independently frozen literals, not constructed from the production resource.
const EN: &[EnglishCase] = &[
    (
        "my thermos is still in the",
        ["backpack", "kitchen"],
        ["backpac", "kitche"],
        [
            "my thermos is still in the backpack next to my lunchbox.",
            "my thermos is still in the kitchen by the sink.",
        ],
    ),
    (
        "i put the chopsticks in the",
        ["pouch", "drawer"],
        ["pouc", "drawe"],
        [
            "i put the chopsticks in the pouch with the spoon.",
            "i put the chopsticks in the drawer with the forks.",
        ],
    ),
    (
        "the spare napkins are in the",
        ["basket", "cupboard"],
        ["baske", "cupboar"],
        [
            "the spare napkins are in the basket next to the cups.",
            "the spare napkins are in the cupboard above the sink.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "bao'wen'bei",
        "bao'wen'be",
        "保温杯",
        ["保温杯放在背包的侧袋里。", "保温杯还在厨房的架子上。"],
    ),
    (
        "can'ju",
        "can'j",
        "餐具",
        ["餐具放在抽屉的前面。", "餐具在袋子里，和纸巾放在一起。"],
    ),
    (
        "kuai'zi",
        "kuai'z",
        "筷子",
        ["筷子放在餐具旁边。", "筷子在小盒子里，我帮你拿过来。"],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "suitou",
        "suito",
        "水筒",
        [
            "水筒は鞄の横のポケットに入っています。",
            "水筒を机に置いたままでした。",
        ],
    ),
    (
        "yunomi",
        "yunom",
        "湯飲み",
        [
            "湯飲みは食器棚の手前にあります。",
            "湯飲みを置いたお盆はどこですか。",
        ],
    ),
    (
        "kozara",
        "kozar",
        "小皿",
        [
            "小皿は引き出しの手前にあります。",
            "小皿を入れた箱は棚の上にあります。",
        ],
    ),
];

#[test]
fn table_items_preserves_exact_old_resources_readings_flags_rank_sources_and_authors() {
    for (raw, first, after, hash) in [
        (
            include_str!("../data/lexicons/en.json"),
            ",\n    {\n      \"id\": \"daily_table_items\",",
            "\n  ],\n  \"phrase_endings\": [",
            "9731910e88a5a4d25f0d8b76b4387ceac9a26732a9c7dc4264c6e1b9c1eba16f",
        ),
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            ",\n    [\n      \"保温杯\",",
            "\n  ]\n}",
            "49706aca073e82be1c619c911a0177f9317050ffda8a7651ef63125c24e09592",
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            ",\n    [\n      \"水筒\",",
            "\n  ]\n}",
            "119355914f5df3f2c73d2423f7e78c4ed833f81547b7ca09fcfafe7212a107ba",
        ),
    ] {
        SUITE.preserve_resource(
            &living_room_reading::without_living_room_reading(raw),
            first,
            after,
            hash,
        );
    }
}

#[test]
fn table_items_adds_only_three_contexts_six_authors_per_language_and_no_forms_or_readings() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 63);
    assert_eq!(en.next_words().count(), 915);
    assert_eq!(en.sentences().count(), 1794);
    let layer = &en.word_layers()[60];
    assert_eq!(layer.id, "daily_table_items");
    assert!(layer.words.is_empty());
    assert_eq!((layer.next_words.len(), layer.sentences.len()), (3, 6));
    let known: HashSet<_> = en.word_layers()[..60]
        .iter()
        .flat_map(|l| {
            l.words
                .iter()
                .chain(l.next_words.iter().flat_map(|(_, ws)| ws))
                .chain(&l.sentences)
        })
        .flat_map(|s| s.split(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '\'' | '’'))))
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('’', "'").to_ascii_lowercase())
        .collect();
    assert_eq!(known.len(), 6213);
    for text in layer
        .sentences
        .iter()
        .chain(layer.next_words.iter().flat_map(|(_, ws)| ws))
    {
        for token in text.split(|c: char| !(c.is_ascii_alphabetic() || c == '\'')) {
            assert!(
                token.is_empty() || known.contains(token),
                "new form: {token}"
            );
        }
    }
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[i].0, context);
        assert_eq!(layer.next_words[i].1, words);
        assert_eq!(&layer.sentences[2 * i..2 * i + 2], authors);
    }
    for (language, entries, reads, contexts, authors, old_contexts) in [
        ("zh-Hans", ZH, 3442, 906, 1810, 885),
        ("ja", JA, 638, 394, 787, 373),
    ] {
        let data = builtin(language).unwrap();
        assert_eq!(data.readings().len(), reads);
        assert_eq!(data.continuations().count(), contexts);
        assert_eq!(
            data.continuations().map(|(_, vs)| vs.len()).sum::<usize>(),
            authors
        );
        let tail: Vec<_> = data.continuations().skip(old_contexts).take(3).collect();
        assert_eq!(tail.len(), 3);
        for (i, &(_, _, word, pair)) in entries.iter().enumerate() {
            assert_eq!(tail[i].0, word);
            assert_eq!(tail[i].1, pair);
            assert!(data.readings().iter().any(|r| r.text == word));
        }
    }
}

#[test]
fn table_items_english_full_partials_and_all_six_branches_commit_exactly_once() {
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
fn table_items_chinese_joined_separated_case_tones_and_partials_offer_both_authors() {
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
    for (i, toned) in ["bao3'wen1'bei1", "can1'ju4", "kuai4'zi5"]
        .into_iter()
        .enumerate()
    {
        SUITE.choices("zh-Hans", toned, ZH[i].2, &ZH[i].3);
    }
}

#[test]
fn table_items_japanese_fixed_kana_romaji_case_and_partials_preserve_raw_suito() {
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["すいとう", "ゆのみ", "こざら"][i];
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
    assert_eq!(SUITE.engine("ja", "suito").snapshot().seed_text, "suito");
    SUITE.commit("ja", "suito", "suito");
}

#[test]
fn table_items_six_horizontal_spaces_adoption_and_long_drafts_keep_exact_prefixes() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = format!("the{gap}spare{gap}napkins{gap}are{gap}in{gap}the{gap}baske");
        let word = seed.strip_suffix("baske").unwrap().to_owned() + "basket";
        let author = word.clone() + " next to the cups.";
        SUITE.choices("en", &seed, &word, &[&author]);
        let seed = format!("bao{gap}wen{gap}bei");
        SUITE.choices("zh-Hans", &seed, ZH[0].2, &ZH[0].3);
        SUITE.commit("zh-Hans", &seed, &seed);
        for (left, right, which) in [("sui", "tou", 0), ("yu", "nomi", 1), ("ko", "zara", 2)] {
            let seed = format!("{left}{gap}{right}");
            SUITE.choices("ja", &seed, JA[which].2, &JA[which].3);
            SUITE.commit("ja", &seed, &seed);
        }
    }
    SUITE.adopt("zh-Hans", ZH[0].1, ZH[0].2, &ZH[0].3);
    SUITE.adopt("ja", "suito", JA[0].2, &JA[0].3);
    let prefix = "之前的记录🙂，不改写已有内容。".repeat(30);
    let seed = format!("{prefix}the spare napkins are in the baske");
    let word = format!("{prefix}the spare napkins are in the basket");
    let author = format!("{word} next to the cups.");
    SUITE.choices("en", &seed, &word, &[&author]);
    SUITE.commit("en", &seed, &author);
}

#[test]
fn table_items_raw_rescue_does_not_join_seven_line_boundaries_or_opaque_tokens() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, author) in [
            (
                "en",
                format!("the spare napkins are{line}in the baske"),
                EN[2].3[0],
            ),
            ("zh-Hans", format!("bao{line}wenbei"), ZH[0].3[0]),
            ("ja", format!("sui{line}tou"), JA[0].3[0]),
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
        ("en", "https://example.invalid/my_thermos/backpac"),
        ("en", "spare_napkins_baske"),
        ("zh-Hans", "user_bao_wen_bei_42"),
        ("ja", "user_suitou_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
}

#[test]
fn table_items_pending_disabled_five_typed_failures_and_empty_models_keep_full_local_pool() {
    SUITE.missing_models(
        "en",
        "my thermos is still in the backpac",
        "my thermos is still in the backpack",
        &EN[0].3[..1],
    );
    SUITE.missing_models("zh-Hans", "bao'wen'be", ZH[0].2, &ZH[0].3);
    SUITE.missing_models("ja", "suito", JA[0].2, &JA[0].3);
}

fn old_controls(language: &str) {
    match language {
        "en" => SUITE.choices(
            "en",
            "please leave the towel he",
            "please leave the towel here",
            &["please leave the towel here by the sink."],
        ),
        "zh-Hans" => SUITE.choices(
            "zh-Hans",
            "ya'gao",
            "牙膏",
            &["牙膏我放在牙刷旁边了。", "牙膏快用完了，我先记在清单上。"],
        ),
        "ja" => SUITE.choices(
            "ja",
            "hamigaki",
            "歯磨き",
            &[
                "歯磨きを済ませてから出かけます。",
                "歯磨きはもう終わりました。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn table_items_preserves_old_care_hygiene_and_nasal_author_controls() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn table_items_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs() {
    const NAME: &str =
        "table_items_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_TABLE_ITEMS_TEST_CHILD",
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

//! Fixed personal-item dialogue; no sensing, tracking or device control.
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
    marker: "PERSONAL_ITEMS",
};
// Independently frozen literals, not constructed from the production resource.
const EN: &[EnglishCase] = &[
    (
        "my scarf is still on the",
        ["chair", "rack"],
        ["chai", "rac"],
        [
            "my scarf is still on the chair by the door.",
            "my scarf is still on the rack next to my coat.",
        ],
    ),
    (
        "the gloves are together in the",
        ["drawer", "basket"],
        ["drawe", "baske"],
        [
            "the gloves are together in the drawer with my scarf.",
            "the gloves are together in the basket next to my bag.",
        ],
    ),
    (
        "i left my cap on the",
        ["shelf", "chair"],
        ["shel", "chai"],
        [
            "i left my cap on the shelf above my shoes.",
            "i left my cap on the chair by the bed.",
        ],
    ),
    (
        "my belt is still in the",
        ["wardrobe", "drawer"],
        ["wardrob", "drawe"],
        [
            "my belt is still in the wardrobe next to my coat.",
            "my belt is still in the drawer with my gloves.",
        ],
    ),
    (
        "i put my watch in the",
        ["pouch", "drawer"],
        ["pouc", "drawe"],
        [
            "i put my watch in the pouch next to my keys.",
            "i put my watch in the drawer with the spare key.",
        ],
    ),
    (
        "the spare buttons are in the",
        ["box", "pouch"],
        ["bo", "pouc"],
        [
            "the spare buttons are in the box on the shelf.",
            "the spare buttons are in the pouch in the top drawer.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "shu'zi",
        "shu'z",
        "梳子",
        ["梳子放在洗漱包的小袋里。", "梳子在镜子旁边，我帮你拿过来。"],
    ),
    (
        "jing'zi",
        "jing'z",
        "镜子",
        ["镜子放在桌边的小盒子里。", "镜子我放回洗漱包里了。"],
    ),
    (
        "yao'dai",
        "yao'da",
        "腰带",
        ["腰带放在衣柜的抽屉里。", "腰带在围巾下面，我帮你拿过来。"],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "tebukuro",
        "tebukur",
        "手袋",
        [
            "手袋は上着のポケットに入っています。",
            "手袋を入れた袋は玄関の棚にあります。",
        ],
    ),
    (
        "hankachi",
        "hankach",
        "ハンカチ",
        [
            "ハンカチは鞄の内ポケットにあります。",
            "ハンカチの予備は引き出しにあります。",
        ],
    ),
    (
        "ki-ke-su",
        "ki-ke-s",
        "キーケース",
        [
            "キーケースは玄関の小さなかごにあります。",
            "キーケースを上着のポケットに入れてあります。",
        ],
    ),
];

#[test]
fn personal_items_preserves_exact_old_resources_readings_flags_rank_sources_and_authors() {
    for (raw, first, after, hash) in [
        (
            include_str!("../data/lexicons/en.json"),
            ",\n    {\n      \"id\": \"daily_essentials_dialogue\",",
            "\n  ],\n  \"phrase_endings\": [",
            "cdcd49c76d7f7c7575dbe5d831a26c90a00db39f054d5c96ad71b792f5b73e5d",
        ),
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            ",\n    [\n      \"梳子\",",
            "\n  ]\n}",
            "17cf01cc061faaea9f247a4713deb3cfaf3cc2048e9d14a2f64e04407c8d5a70",
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            ",\n    [\n      \"手袋\",",
            "\n  ]\n}",
            "be1c068f5cf7fc624099707fb95d283727fe43500cd8ff3ab8f556998f4cdcc9",
        ),
    ] {
        SUITE.preserve_resource(raw, first, after, hash);
    }
}

#[test]
fn personal_items_adds_twelve_contexts_twenty_four_authors_without_new_forms_or_readings() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 968);
    assert_eq!(en.sentences().count(), 1900);
    let layer = &en.word_layers()[62];
    assert_eq!(layer.id, "daily_essentials_dialogue");
    assert!(layer.words.is_empty());
    assert!(layer.next_words.len() >= 6 && layer.sentences.len() >= 12);
    let known: HashSet<_> = en.word_layers()[..62]
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
    // Freeze this historical batch's own six contexts / twelve authors. Later
    // append-only themes have independent exact-delta/raw-resource guards.
    for text in layer
        .sentences
        .iter()
        .take(12)
        .chain(layer.next_words.iter().take(6).flat_map(|(_, ws)| ws))
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
        ("zh-Hans", ZH, 3442, 959, 1916, 891),
        ("ja", JA, 638, 447, 893, 379),
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
fn personal_items_historical_reading_inverse_freezes_all_three_old_context_records() {
    let raw = include_str!("../data/lexicons/zh-Hans.json");
    let projected = living_room_reading::without_living_room_reading(raw);
    let marker = "\n  ],\n  \"continuations\": [";
    assert_eq!(
        &raw[raw.find(marker).unwrap()..],
        &projected[projected.find(marker).unwrap()..]
    );
    let previous: serde_json::Value = serde_json::from_str(&projected).unwrap();
    assert_eq!(previous["readings"].as_array().unwrap().len(), 3441);
    assert_eq!(previous["continuations"].as_array().unwrap().len(), 959);
    // These fixed old strings test the inverse itself, not candidate discovery.
    for old in [
        "\"遥控器\",\n      [",
        "遥控器放在沙发旁边的小桌上。",
        "遥控器在纸巾盒后面，我刚才看见了。",
        "\"纸巾盒\",\n      [",
        "纸巾盒放在茶几的右边。",
        "纸巾盒在电视柜旁边，我帮你拿过来。",
        "\"眼镜盒\",\n      [",
        "眼镜盒放在靠窗的小桌上。",
        "眼镜盒在抽屉的前面，你看看。",
    ] {
        assert_eq!(raw.matches(old).count(), 1);
        let changed = if old.ends_with('[') {
            old.replacen("\",", "改\",", 1)
        } else {
            old.to_owned() + "改"
        };
        let bad = raw.replace(old, &changed);
        // Valid context-only mutations must fail, not just JSON syntax errors.
        assert!(serde_json::from_str::<serde_json::Value>(&bad).is_ok());
        assert!(
            std::panic::catch_unwind(|| living_room_reading::without_living_room_reading(&bad))
                .is_err()
        );
    }
    // Historical author order is part of the identity, not just membership.
    let (first, second) = (
        "遥控器放在沙发旁边的小桌上。",
        "遥控器在纸巾盒后面，我刚才看见了。",
    );
    assert!(!raw.contains("__SWAP_HISTORICAL_AUTHOR__"));
    let swapped = raw
        .replace(first, "__SWAP_HISTORICAL_AUTHOR__")
        .replace(second, first)
        .replace("__SWAP_HISTORICAL_AUTHOR__", second);
    assert!(serde_json::from_str::<serde_json::Value>(&swapped).is_ok());
    assert!(
        std::panic::catch_unwind(|| living_room_reading::without_living_room_reading(&swapped))
            .is_err()
    );
}

#[test]
fn personal_items_english_full_partials_and_all_twelve_branches_commit_exactly_once() {
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
fn personal_items_chinese_joined_separated_case_tones_and_partials_offer_both_authors() {
    for &(reading, partial, word, authors) in ZH {
        for seed in [
            reading.into(),
            reading.replace('\'', ""),
            reading.replace('\'', " ").to_ascii_uppercase(),
            partial.into(),
        ] {
            if seed == "yao'da" {
                // This genuine short-prefix collision retains the old “要大”.
                // The second author is deliberately verified on page two.
                let ime = SUITE.engine("zh-Hans", &seed);
                assert_eq!(ime.candidates()[0].text, "要大");
                SUITE.first_page(&ime, word, CandidateKind::Word);
                for (author, expected) in authors.into_iter().zip([5, 6]) {
                    assert_eq!(
                        ime.candidates().iter().position(|c| {
                            c.text == author && c.kind == CandidateKind::Sentence
                        }),
                        Some(expected)
                    );
                }
            } else {
                SUITE.choices("zh-Hans", &seed, word, &authors);
            }
            for text in [word, authors[0], authors[1], &seed] {
                SUITE.commit("zh-Hans", &seed, text);
            }
        }
    }
    for (i, toned) in ["shu1'zi5", "jing4'zi5", "yao1'dai4"]
        .into_iter()
        .enumerate()
    {
        SUITE.choices("zh-Hans", toned, ZH[i].2, &ZH[i].3);
    }
}

#[test]
fn personal_items_japanese_fixed_kana_romaji_case_and_partials_preserve_raw_spelling() {
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["てぶくろ", "はんかち", "きーけーす"][i];
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
        ("tebukur", "てぶくr"),
        ("hankach", "はんかch"),
        ("ki-ke-s", "きーけーs"),
    ] {
        assert_eq!(romaji_to_hiragana(raw), kana);
        assert_eq!(SUITE.engine("ja", raw).snapshot().seed_text, raw);
        SUITE.commit("ja", raw, raw);
    }
}

#[test]
fn personal_items_six_horizontal_spaces_adoption_and_long_drafts_keep_exact_prefixes() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = format!("the{gap}gloves{gap}are{gap}together{gap}in{gap}the{gap}baske");
        let word = seed.strip_suffix("baske").unwrap().to_owned() + "basket";
        let author = word.clone() + " next to my bag.";
        SUITE.choices("en", &seed, &word, &[&author]);
        for (i, &(reading, _, _, _)) in ZH.iter().enumerate() {
            let seed = reading.replace('\'', gap);
            SUITE.choices("zh-Hans", &seed, ZH[i].2, &ZH[i].3);
            SUITE.commit("zh-Hans", &seed, &seed);
        }
        for (i, (left, right)) in [("te", "bukuro"), ("han", "kachi"), ("ki-", "ke-su")]
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
    let seed = format!("{prefix}the gloves are together in the baske");
    let word = format!("{prefix}the gloves are together in the basket");
    let author = format!("{word} next to my bag.");
    SUITE.choices("en", &seed, &word, &[&author]);
    SUITE.commit("en", &seed, &author);
}

#[test]
fn personal_items_raw_rescue_does_not_join_seven_line_boundaries_or_opaque_tokens() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, author) in [
            (
                "en",
                format!("my scarf is still{line}on the chai"),
                EN[0].3[0],
            ),
            ("zh-Hans", format!("shu{line}zi"), ZH[0].3[0]),
            ("ja", format!("te{line}bukuro"), JA[0].3[0]),
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
        ("en", "https://example.invalid/my_scarf/chai"),
        ("en", "my_scarf_chai_42"),
        ("zh-Hans", "user_shu_zi_42"),
        ("ja", "user_tebukuro_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
}

#[test]
fn personal_items_pending_disabled_five_typed_failures_and_empty_models_keep_full_local_pool() {
    SUITE.missing_models(
        "en",
        "my scarf is still on the chai",
        "my scarf is still on the chair",
        &EN[0].3[..1],
    );
    SUITE.missing_models("zh-Hans", "shu'z", ZH[0].2, &ZH[0].3);
    SUITE.missing_models("ja", "tebukur", JA[0].2, &JA[0].3);
}

fn old_controls(language: &str) {
    match language {
        "en" => SUITE.choices(
            "en",
            "the remote is still on the sof",
            "the remote is still on the sofa",
            &["the remote is still on the sofa next to the blanket."],
        ),
        "zh-Hans" => SUITE.choices(
            "zh-Hans",
            "yao'kong'q",
            "遥控器",
            &[
                "遥控器放在沙发旁边的小桌上。",
                "遥控器在纸巾盒后面，我刚才看见了。",
            ],
        ),
        "ja" => SUITE.choices(
            "ja",
            "rimoko",
            "リモコン",
            &[
                "リモコンはソファの横の小さな机にあります。",
                "リモコンを入れたかごは棚の下にあります。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn personal_items_preserves_previous_living_room_author_controls() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn personal_items_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs() {
    const NAME: &str =
        "personal_items_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_PERSONAL_ITEMS_TEST_CHILD",
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

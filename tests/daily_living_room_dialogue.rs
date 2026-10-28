//! Fixed living-room dialogue; no sensing, battery advice or device control.
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
    marker: "LIVING_ROOM",
};
// Independently frozen literals, not constructed from the production resource.
const EN: &[EnglishCase] = &[
    (
        "the remote is still on the",
        ["sofa", "table"],
        ["sof", "tabl"],
        [
            "the remote is still on the sofa next to the blanket.",
            "the remote is still on the table next to the lamp.",
        ],
    ),
    (
        "i put the spare batteries in the",
        ["drawer", "box"],
        ["drawe", "bo"],
        [
            "i put the spare batteries in the drawer by the sofa.",
            "i put the spare batteries in the box with the cables.",
        ],
    ),
    (
        "the magazines are together in the",
        ["basket", "tray"],
        ["baske", "tra"],
        [
            "the magazines are together in the basket by the sofa.",
            "the magazines are together in the tray on the table.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "yao'kong'qi",
        "yao'kong'q",
        "遥控器",
        [
            "遥控器放在沙发旁边的小桌上。",
            "遥控器在纸巾盒后面，我刚才看见了。",
        ],
    ),
    (
        "zhi'jin'he",
        "zhi'jin'h",
        "纸巾盒",
        [
            "纸巾盒放在茶几的右边。",
            "纸巾盒在电视柜旁边，我帮你拿过来。",
        ],
    ),
    (
        "yan'jing'he",
        "yan'jing'h",
        "眼镜盒",
        ["眼镜盒放在靠窗的小桌上。", "眼镜盒在抽屉的前面，你看看。"],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "rimokon",
        "rimoko",
        "リモコン",
        [
            "リモコンはソファの横の小さな机にあります。",
            "リモコンを入れたかごは棚の下にあります。",
        ],
    ),
    (
        "megane",
        "megan",
        "眼鏡",
        [
            "眼鏡はリモコンの横にあります。",
            "眼鏡を机に置いたままでした。",
        ],
    ),
    (
        "komono",
        "komon",
        "小物",
        [
            "小物はテーブルの上のかごにまとめてあります。",
            "小物を入れた箱は棚の手前にあります。",
        ],
    ),
];

#[test]
fn living_room_preserves_exact_old_resources_readings_flags_rank_sources_and_authors() {
    for (raw, first, after, hash) in [
        (
            include_str!("../data/lexicons/en.json").to_owned(),
            ",\n    {\n      \"id\": \"daily_living_room\",",
            "\n  ],\n  \"phrase_endings\": [",
            "0093f628ea334d37d6649ab16cafaab4bc6041b310c4ab2785bd0481ccd5d640",
        ),
        (
            living_room_reading::without_living_room_reading(include_str!(
                "../data/lexicons/zh-Hans.json"
            )),
            ",\n    [\n      \"遥控器\",",
            "\n  ]\n}",
            "7e5e0e8cd2f223cd4bd8308399485477ca960980a1272c6fe2b31cec0fcaa490",
        ),
        (
            include_str!("../data/lexicons/ja.json").to_owned(),
            ",\n    [\n      \"リモコン\",",
            "\n  ]\n}",
            "5b46a3322bbb39aba7285793189c04d3945945e33ca0405dd22eb8f84ef2f875",
        ),
    ] {
        SUITE.preserve_resource(&raw, first, after, hash);
    }
}

#[test]
fn living_room_adds_three_contexts_six_authors_per_language_and_one_chinese_word_reading() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 946);
    assert_eq!(en.sentences().count(), 1856);
    let layer = &en.word_layers()[61];
    assert_eq!(layer.id, "daily_living_room");
    assert!(layer.words.is_empty());
    assert_eq!((layer.next_words.len(), layer.sentences.len()), (3, 6));
    let known: HashSet<_> = en.word_layers()[..61]
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
        ("zh-Hans", ZH, 3442, 937, 1872, 888),
        ("ja", JA, 638, 425, 849, 376),
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
fn living_room_reading_projection_rejects_missing_mutated_duplicate_and_misplaced_data() {
    const APPEND: &str =
        ",\n    {\"reading\": \"yao'kong'qi\", \"text\": \"遥控器\", \"kind\": \"word\"}";
    const END: &str = "\n  ],\n  \"continuations\": [";
    let raw = include_str!("../data/lexicons/zh-Hans.json");
    assert_eq!(raw.matches(APPEND).count(), 1);
    let projected = living_room_reading::without_living_room_reading(raw);
    let parsed: serde_json::Value = serde_json::from_str(&projected).unwrap();
    assert_eq!(parsed["readings"].as_array().unwrap().len(), 3441);
    assert_eq!(parsed["continuations"].as_array().unwrap().len(), 937);
    assert_eq!(
        &raw[raw.find(END).unwrap()..],
        &projected[projected.find(END).unwrap()..],
        "reading projection must never change the context bytes"
    );
    let without = raw.replace(APPEND, "");
    let misplaced = without.replace(
        "  \"readings\": [\n",
        "  \"readings\": [\n    {\"reading\": \"yao'kong'qi\", \"text\": \"遥控器\", \"kind\": \"word\"},\n",
    );
    for bad in [
        without,
        raw.replace(APPEND, &(APPEND.to_owned() + APPEND)),
        raw.replace("\"yao'kong'qi\"", "\"yao'kong'q\""),
        raw.replace(APPEND, &APPEND.replace("\"遥控器\"", "\"遥控汽\"")),
        raw.replace(APPEND, &APPEND.replace("\"word\"", "\"sentence\"")),
        raw.replace(
            APPEND,
            &APPEND.replace("\"word\"}", "\"word\", \"require_separators\": false}"),
        ),
        raw.replacen("\"format_version\": 1", "\"format_version\": 2", 1),
        misplaced,
    ] {
        assert!(
            std::panic::catch_unwind(|| living_room_reading::without_living_room_reading(&bad))
                .is_err(),
            "unsafe historical projection accepted a mutated resource"
        );
    }
}

#[test]
fn living_room_keeps_existing_tissue_word_and_authors_next_to_the_new_box_context() {
    SUITE.choices(
        "zh-Hans",
        "zhi'jin",
        "纸巾",
        &["纸巾放在桌上了。", "纸巾快用完了。"],
    );
    for raw in ["megan", "komon"] {
        SUITE.commit("ja", raw, raw);
    }
}

#[test]
fn living_room_english_full_partials_and_all_six_branches_commit_exactly_once() {
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
fn living_room_chinese_joined_separated_case_tones_and_partials_offer_both_authors() {
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
    for (i, toned) in ["yao2'kong4'qi4", "zhi3'jin1'he2", "yan3'jing4'he2"]
        .into_iter()
        .enumerate()
    {
        SUITE.choices("zh-Hans", toned, ZH[i].2, &ZH[i].3);
    }
}

#[test]
fn living_room_japanese_fixed_kana_romaji_case_and_partials_preserve_raw_terminal_n() {
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["りもこん", "めがね", "こもの"][i];
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
        ("rimoko", "りもこ"),
        ("megan", "めがん"),
        ("komon", "こもん"),
    ] {
        assert_eq!(romaji_to_hiragana(raw), kana);
        assert_eq!(SUITE.engine("ja", raw).snapshot().seed_text, raw);
        SUITE.commit("ja", raw, raw);
    }
}

#[test]
fn living_room_six_horizontal_spaces_adoption_and_long_drafts_keep_exact_prefixes() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = format!("the{gap}magazines{gap}are{gap}together{gap}in{gap}the{gap}baske");
        let word = seed.strip_suffix("baske").unwrap().to_owned() + "basket";
        let author = word.clone() + " by the sofa.";
        SUITE.choices("en", &seed, &word, &[&author]);
        for (i, &(reading, _, _, _)) in ZH.iter().enumerate() {
            let seed = reading.replace('\'', gap);
            SUITE.choices("zh-Hans", &seed, ZH[i].2, &ZH[i].3);
            SUITE.commit("zh-Hans", &seed, &seed);
        }
        for (i, (left, right)) in [("rimo", "kon"), ("mega", "ne"), ("komo", "no")]
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
    let seed = format!("{prefix}the magazines are together in the baske");
    let word = format!("{prefix}the magazines are together in the basket");
    let author = format!("{word} by the sofa.");
    SUITE.choices("en", &seed, &word, &[&author]);
    SUITE.commit("en", &seed, &author);
}

#[test]
fn living_room_raw_rescue_does_not_join_seven_line_boundaries_or_opaque_tokens() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, author) in [
            (
                "en",
                format!("the remote is still{line}on the sof"),
                EN[0].3[0],
            ),
            ("zh-Hans", format!("yao{line}kongqi"), ZH[0].3[0]),
            ("ja", format!("rimo{line}kon"), JA[0].3[0]),
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
        ("en", "https://example.invalid/the_remote/sof"),
        ("en", "remote_sof_42"),
        ("zh-Hans", "user_yao_kong_qi_42"),
        ("ja", "user_rimokon_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
}

#[test]
fn living_room_pending_disabled_five_typed_failures_and_empty_models_keep_full_local_pool() {
    SUITE.missing_models(
        "en",
        "the remote is still on the sof",
        "the remote is still on the sofa",
        &EN[0].3[..1],
    );
    SUITE.missing_models("zh-Hans", "yao'kong'q", ZH[0].2, &ZH[0].3);
    SUITE.missing_models("ja", "rimoko", JA[0].2, &JA[0].3);
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
fn living_room_preserves_old_care_hygiene_and_nasal_author_controls() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn living_room_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs() {
    const NAME: &str =
        "living_room_old_and_new_choices_coexist_with_private_8_8_9_recommended_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_LIVING_ROOM_TEST_CHILD",
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

//! Finite conversation closings and visit farewells, not presence tracking or calendar execution.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/farewell_resource.rs"]
mod farewell_resource;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::BTreeMap;
use suzaku_map::{
    ime::candidate_mix::CandidateKind,
    languages::japanese::romaji_to_hiragana,
    lexicon::{Lexicon, WordLayer, builtin},
};

const SUITE: Suite = Suite { marker: "FAREWELL" };
// Authored expectations are fixed independently of resource parsing and candidate output.
const EN: &[EnglishCase] = &[
    (
        "i should let you get back to",
        ["work", "dinner"],
        ["wor", "dinne"],
        [
            "i should let you get back to work now, thanks for the chat.",
            "i should let you get back to dinner, we'll talk another time.",
        ],
    ),
    (
        "i have to go now, but let's",
        ["keep", "talk"],
        ["kee", "tal"],
        [
            "i have to go now, but let's keep in touch.",
            "i have to go now, but let's talk again soon.",
        ],
    ),
    (
        "thanks for the chat, have a",
        ["good", "safe"],
        ["goo", "saf"],
        [
            "thanks for the chat, have a good evening.",
            "thanks for the chat, have a safe trip home.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "xian'zhe'yang'ba",
        "xian'zhe'yang'b",
        "先这样吧",
        [
            "先这样吧，今天就聊到这里。",
            "先这样吧，你早点休息，我们改天再聊。",
        ],
    ),
    (
        "xia'ci'zai'liao",
        "xia'ci'zai'lia",
        "下次再聊",
        [
            "下次再聊，今天和你聊得很开心。",
            "下次再聊，有空再跟我说说后来的事。",
        ],
    ),
    (
        "bao'chi'lian'xi",
        "bao'chi'lian'x",
        "保持联系",
        [
            "保持联系，哪天有空再聊聊近况。",
            "保持联系，有新消息随时告诉我。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "ojamashimashita",
        "ojamashimashit",
        "お邪魔しました",
        [
            "お邪魔しました。今日はゆっくりお話しできて楽しかったです。",
            "お邪魔しました。またお会いできるのを楽しみにしています。",
        ],
    ),
    (
        "okage",
        "okag",
        "おかげ",
        [
            "おかげで楽しくお話しできました。今日はありがとうございました。",
            "おかげで疑問が解けました。またお話しできたらうれしいです。",
        ],
    ),
    (
        "miokuri",
        "miokur",
        "見送り",
        [
            "見送りはここまでで大丈夫です。またゆっくり話しましょう。",
            "見送りに来てくれてありがとう。また連絡するね。",
        ],
    ),
];
const TONES: [&str; 3] = [
    "xian1'zhe4'yang4'ba5",
    "xia4'ci4'zai4'liao2",
    "bao3'chi2'lian2'xi4",
];
const KANA: [&str; 3] = ["おじゃましました", "おかげ", "みおくり"];
const PARTIAL_KANA: [&str; 3] = ["おじゃましましt", "おかg", "みおくr"];
const RESOURCES: &[(&str, &str)] = &[
    (
        include_str!("../data/lexicons/en.json"),
        "591123aac758c7ccd1e75f711aa582af016188583a1e769e879d1da3ee3fcba6",
    ),
    (
        include_str!("../data/lexicons/zh-Hans.json"),
        "1cdb49a044a5043e398913cccbfba214d5d06feef0f7ce9a0262ff0dd35f1e66",
    ),
    (
        include_str!("../data/lexicons/ja.json"),
        "785687c646f1604351c857419a2501796f8d8fcf96e09a9d54cba48bf0477aa1",
    ),
];

fn probe(language: &str, seed: &str, word: &str, authors: &[&str]) {
    SUITE.choices(language, seed, word, authors);
    for text in [seed, word].into_iter().chain(authors.iter().copied()) {
        SUITE.commit(language, seed, text);
    }
}

fn ranks(layers: &[WordLayer]) -> BTreeMap<String, usize> {
    let mut output = BTreeMap::new();
    let mut rank = 0;
    for layer in layers {
        for word in layer
            .words
            .iter()
            .map(String::as_str)
            .chain(
                layer
                    .next_words
                    .iter()
                    .flat_map(|(_, words)| words.iter().map(String::as_str)),
            )
            .chain(layer.sentences.iter().flat_map(|s| {
                s.split(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '\'' | '’')))
                    .filter(|word| !word.is_empty())
            }))
        {
            output
                .entry(word.replace('’', "'").to_ascii_lowercase())
                .or_insert(rank);
            rank += 1;
        }
    }
    output
}

#[test]
fn farewell_appends_nine_contexts_eighteen_authors_with_exact_consideration_inverse() {
    for &(raw, expected_sha) in RESOURCES {
        let previous = farewell_resource::without_farewell(raw);
        SUITE.preserve_resource(&previous, "absent-farewell", "absent-end", expected_sha);
        assert_eq!(farewell_resource::without_farewell(&previous), previous);
    }
    let en = builtin("en").unwrap();
    assert_eq!(
        (
            en.word_layers().len(),
            en.next_words().count(),
            en.sentences().count()
        ),
        (64, 946, 1856)
    );
    let layer = &en.word_layers()[63];
    assert_eq!(layer.id, "daily_presence_dialogue");
    assert!(layer.words.is_empty());
    assert_eq!((layer.next_words.len(), layer.sentences.len()), (31, 62));
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[18 + i].0, context);
        assert_eq!(layer.next_words[18 + i].1, words);
        assert_eq!(&layer.sentences[36 + 2 * i..38 + 2 * i], authors);
    }
    for (language, entries, readings, start, contexts, authors) in [
        ("zh-Hans", ZH, 3442, 924, 937, 1872),
        ("ja", JA, 638, 412, 425, 849),
    ] {
        let data = builtin(language).unwrap();
        assert_eq!(
            (data.readings().len(), data.continuations().count()),
            (readings, contexts)
        );
        assert_eq!(
            data.continuations()
                .map(|(_, pair)| pair.len())
                .sum::<usize>(),
            authors
        );
        for ((word, pair), &(_, _, expected, authors)) in
            data.continuations().skip(start).take(3).zip(entries)
        {
            assert_eq!(word, expected);
            assert_eq!(pair, authors);
        }
    }
}

#[test]
fn farewell_inverse_rejects_changed_old_bytes_mutations_removals_and_unknown_future() {
    for (&(raw, _), new_author) in RESOURCES.iter().zip([EN[0].3[0], ZH[0].3[0], JA[0].3[0]]) {
        let changed = raw.replacen("\"language\": ", "\"language\":  ", 1);
        assert_ne!(changed, raw);
        assert!(
            std::panic::catch_unwind(|| farewell_resource::without_farewell(&changed)).is_err()
        );
        let quoted = serde_json::to_string(new_author).unwrap();
        if !raw.contains(&quoted) {
            continue; // Exact previous resources remain valid before the RED append.
        }
        let changed = raw.replacen(
            &quoted,
            &serde_json::to_string("changed authored tail").unwrap(),
            1,
        );
        assert!(
            std::panic::catch_unwind(|| farewell_resource::without_farewell(&changed)).is_err()
        );
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        for mutation in 0..3 {
            let mut changed = value.clone();
            if value["language"] == "en" {
                let layer = &mut changed["word_layers"][63];
                match mutation {
                    0 => {
                        layer["next_words"].as_array_mut().unwrap().remove(18);
                    }
                    1 => {
                        layer["sentences"].as_array_mut().unwrap().remove(36);
                    }
                    _ => layer["next_words"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(["future context", ["future"]])),
                }
            } else {
                let start = if value["language"] == "ja" { 412 } else { 924 };
                let rows = changed["continuations"].as_array_mut().unwrap();
                match mutation {
                    0 => {
                        rows.remove(start);
                    }
                    1 => {
                        rows[start][1].as_array_mut().unwrap().remove(0);
                    }
                    _ => rows.push(serde_json::json!(["future context", ["future author"]])),
                }
            }
            let changed = serde_json::to_string_pretty(&changed).unwrap() + "\n";
            assert!(
                std::panic::catch_unwind(|| farewell_resource::without_farewell(&changed)).is_err()
            );
        }
    }
}

#[test]
fn farewell_keeps_all_6216_published_english_first_seen_ranks() {
    let previous = farewell_resource::without_farewell(RESOURCES[0].0);
    let previous = Lexicon::from_json(&previous).unwrap();
    let current = builtin("en").unwrap();
    let before = ranks(previous.word_layers());
    let after = ranks(current.word_layers());
    assert_eq!(before.len(), 6216);
    assert_eq!(before, after);
    let known = ranks(&previous.word_layers()[..63]);
    for &(context, words, _, authors) in EN {
        for text in [context, words[0], words[1], authors[0], authors[1]] {
            for token in text
                .split(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '\'' | '’')))
                .filter(|word| !word.is_empty())
            {
                assert!(
                    known.contains_key(&token.replace('’', "'").to_ascii_lowercase()),
                    "new indexed token: {token:?}"
                );
            }
        }
    }
    let bytes = after
        .iter()
        .map(|(word, rank)| format!("{word}\0{rank}\n"))
        .collect::<String>();
    let hash = digest(&SHA256, bytes.as_bytes())
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
fn farewell_english_six_branches_partial_full_and_literal_commit_once() {
    for &(context, words, partials, authors) in EN {
        for i in 0..2 {
            let word = format!("{context} {}", words[i]);
            for seed in [
                format!("{context} "),
                format!("{context} {}", partials[i]),
                word.clone(),
            ] {
                probe("en", &seed, &word, &[authors[i]]);
            }
        }
    }
}

#[test]
fn farewell_chinese_joined_separated_tones_uppercase_han_and_partials() {
    for (i, &(reading, partial, word, authors)) in ZH.iter().enumerate() {
        let tone = TONES[i];
        for seed in [
            reading.into(),
            reading.replace('\'', ""),
            reading.replace('\'', " ").to_ascii_uppercase(),
            partial.into(),
            word.into(),
            tone.into(),
            tone.replace('\'', ""),
            tone.replace('\'', " ").to_ascii_uppercase(),
        ] {
            probe("zh-Hans", &seed, word, &authors);
        }
    }
}

#[test]
fn farewell_japanese_romaji_kana_case_full_and_partial_with_literal_raw() {
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        assert_eq!(romaji_to_hiragana(reading), KANA[i]);
        assert_eq!(romaji_to_hiragana(partial), PARTIAL_KANA[i]);
        for seed in [
            reading.into(),
            reading.to_ascii_uppercase(),
            KANA[i].into(),
            partial.into(),
            partial.to_ascii_uppercase(),
            PARTIAL_KANA[i].into(),
            word.into(),
        ] {
            probe("ja", &seed, word, &authors);
            assert!(
                SUITE
                    .engine("ja", &seed)
                    .candidates()
                    .iter()
                    .any(|c| c.text == seed && c.kind == CandidateKind::Literal)
            );
        }
    }
}

#[test]
fn farewell_six_horizontal_gaps_keep_finite_authored_choices() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = ["i", "should", "let", "you", "get", "back", "to", "wor"].join(gap);
        let word = seed.strip_suffix("wor").unwrap().to_owned() + "work";
        probe(
            "en",
            &seed,
            &word,
            &[&(word.clone() + " now, thanks for the chat.")],
        );
        for &(reading, _, word, authors) in ZH {
            probe("zh-Hans", &reading.replace('\'', gap), word, &authors);
        }
        for (i, (a, b)) in [("ojama", "shimashita"), ("oka", "ge"), ("mioku", "ri")]
            .into_iter()
            .enumerate()
        {
            probe("ja", &format!("{a}{gap}{b}"), JA[i].2, &JA[i].3);
        }
    }
}

#[test]
fn farewell_adoption_and_long_english_draft_preserve_uncommitted_prefix() {
    for (language, entries) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(reading, _, word, authors) in entries {
            SUITE.adopt(language, reading, word, &authors);
        }
    }
    let prefix = "前面的文字🙂，保持原样。".repeat(35);
    let seed = format!("{prefix}i should let you get back to wor");
    let word = format!("{prefix}i should let you get back to work");
    probe(
        "en",
        &seed,
        &word,
        &[&(word.clone() + " now, thanks for the chat.")],
    );
}

#[test]
fn farewell_seven_line_breaks_and_opaque_tokens_keep_literal_raw_rescue() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, authors) in [
            (
                "en",
                format!("i should let you{line}get back to wor"),
                EN[0].3,
            ),
            ("zh-Hans", format!("xian{line}zhe yang ba"), ZH[0].3),
            ("ja", format!("ojama{line}shimashita"), JA[0].3),
        ] {
            assert!(
                !SUITE
                    .engine(language, &seed)
                    .candidates()
                    .iter()
                    .any(|c| authors.contains(&c.text.as_str()))
            );
            SUITE.commit(language, &seed, &seed);
        }
    }
    for (language, seed) in [
        ("en", "i should let you get back to field_wor"),
        (
            "en",
            "https://example.invalid/i_should_let_you_get_back_to_wor",
        ),
        ("zh-Hans", "user_xian_zhe_yang_ba_42"),
        ("ja", "user_ojamashimashita_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
    for seed in [
        "i should let you get back to worz",
        "i should let you get back to dinnz",
        "i have to go now, but let's keez",
        "i have to go now, but let's talz",
        "thanks for the chat, have a gooz",
        "thanks for the chat, have a safz",
    ] {
        assert!(
            !SUITE
                .engine("en", seed)
                .candidates()
                .iter()
                .any(|c| EN.iter().any(|&(context, words, _, authors)| {
                    authors.contains(&c.text.as_str())
                        || words
                            .iter()
                            .any(|word| c.text == format!("{context} {word}"))
                }))
        );
        SUITE.commit("en", seed, seed);
    }
}

#[test]
fn farewell_synthetic_pending_disabled_errors_and_empty_models_keep_local_choices() {
    SUITE.missing_models(
        "en",
        "i should let you get back to wor",
        "i should let you get back to work",
        &EN[0].3[..1],
    );
    SUITE.missing_models("zh-Hans", ZH[0].1, ZH[0].2, &ZH[0].3);
    SUITE.missing_models("ja", JA[0].1, JA[0].2, &JA[0].3);
}

fn old_controls(language: &str) {
    match language {
        "en" => {
            for (seed, word, author) in [
                (
                    "thanks for the offer, but i'd rather wai",
                    "thanks for the offer, but i'd rather wait",
                    "thanks for the offer, but i'd rather wait until later.",
                ),
                (
                    "you can take your time and repl",
                    "you can take your time and reply",
                    "you can take your time and reply when you are free.",
                ),
                (
                    "please don't feel bad about sayin",
                    "please don't feel bad about saying",
                    "please don't feel bad about saying no if you are busy.",
                ),
            ] {
                probe(language, seed, word, &[author]);
            }
        }
        "zh-Hans" => probe(
            language,
            "xin'yi'ling'le",
            "心意领了",
            &["心意领了，这次就不麻烦你了。", "心意领了，东西你留着用吧。"],
        ),
        "ja" => probe(
            language,
            "yukkuri",
            "ゆっくり",
            &[
                "ゆっくりで大丈夫です。急がなくても構いません。",
                "ゆっくり考えてから、返事をもらえれば大丈夫です。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn farewell_previous_neighbor_controls_still_commit_once() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn farewell_old_and_new_choices_coexist_with_private_8_8_9_packs() {
    const NAME: &str = "farewell_old_and_new_choices_coexist_with_private_8_8_9_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_FAREWELL_DIALOGUE_TEST_CHILD",
        old_controls,
        |language| {
            if language == "en" {
                for &(context, words, partials, authors) in EN {
                    for i in 0..2 {
                        let word = format!("{context} {}", words[i]);
                        for seed in [format!("{context} {}", partials[i]), word.clone()] {
                            probe(language, &seed, &word, &[authors[i]]);
                        }
                    }
                }
            } else {
                for &(reading, partial, word, authors) in if language == "ja" { JA } else { ZH } {
                    for seed in [reading, partial, word] {
                        probe(language, seed, word, &authors);
                    }
                }
            }
        },
    );
}

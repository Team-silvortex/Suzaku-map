//! Finite authored alternatives, preferences and negotiation turns, with unchanged codec boundaries.
#[path = "support/alternatives_resource.rs"]
mod alternatives_resource;
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::{BTreeMap, BTreeSet};
use suzaku_map::{
    ime::candidate_mix::CandidateKind,
    languages::japanese::romaji_to_hiragana,
    lexicon::{Lexicon, builtin},
};

const SUITE: Suite = Suite {
    marker: "ALTERNATIVES",
};
// These finite authored cases are independent of candidate output and resource parsing.
const EN: &[EnglishCase] = &[
    (
        "if that does not work, we could",
        ["try", "wait"],
        ["tr", "wai"],
        [
            "if that does not work, we could try a different place.",
            "if that does not work, we could wait until tomorrow.",
        ],
    ),
    (
        "would you be happy with",
        ["tea", "coffee"],
        ["te", "coffe"],
        [
            "would you be happy with tea instead of coffee?",
            "would you be happy with coffee while we wait?",
        ],
    ),
    (
        "we could go with the",
        ["first", "second"],
        ["firs", "secon"],
        [
            "we could go with the first option if that works for you.",
            "we could go with the second option and leave a little later.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "bei'xuan",
        "bei'xua",
        "备选",
        [
            "备选方案先留着，等大家确认后再决定。",
            "备选的时间也发给我吧。",
        ],
    ),
    (
        "ti'dai",
        "ti'da",
        "替代",
        ["替代方案还有别的吗？", "替代的材料要提前准备好。"],
    ),
    (
        "shang'liang",
        "shang'lian",
        "商量",
        [
            "商量一下再决定，大家都说说想法吧。",
            "商量好了，我再告诉你。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "suki",
        "suk",
        "好き",
        [
            "好きな方を選んでください。",
            "好きな色があれば教えてください。",
        ],
    ),
    (
        "katahou",
        "kataho",
        "片方",
        [
            "片方だけ先に試してもいいですか。",
            "片方を選ぶなら、こちらが使いやすそうです。",
        ],
    ),
    (
        "kimeru",
        "kimer",
        "決める",
        [
            "決める前に、みんなの希望も聞きたいです。",
            "決めるのは明日でも大丈夫です。",
        ],
    ),
];

#[test]
fn alternatives_appends_nine_contexts_eighteen_authors_with_exact_rephrasing_inverse() {
    for (raw, previous_sha) in [
        (
            include_str!("../data/lexicons/en.json"),
            "95e1af47438a06ed7ee9e03209eb1248b6cf18c225e2ffb9cf89ec5098a4e579",
        ),
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            "7acb1698962a1a6720793bde15d3766f96d1d6fbe8d0040914939e0800980bbe",
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            "3e70cd7f7ed26dfd1c38d5e5fc889d3f1fcca5af761f3894b9a47acbf407468d",
        ),
    ] {
        let previous = alternatives_resource::without_alternatives(raw);
        // The shared guard checks the complete returned bytes, with no projection.
        SUITE.preserve_resource(
            &previous,
            "absent-alternatives-marker",
            "absent-end",
            previous_sha,
        );
        assert_eq!(
            alternatives_resource::without_alternatives(&previous),
            previous
        );
    }
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 930);
    assert_eq!(en.sentences().count(), 1824);
    let layer = &en.word_layers()[63];
    assert_eq!(layer.id, "daily_presence_dialogue");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 15);
    assert_eq!(layer.sentences.len(), 30);
    assert_eq!(layer.next_words[9..12].len(), EN.len());
    assert_eq!(layer.sentences[18..24].len(), EN.len() * 2);
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[9 + i].0, context);
        assert_eq!(layer.next_words[9 + i].1, words);
        assert_eq!(&layer.sentences[18 + 2 * i..20 + 2 * i], authors);
    }
    for (language, entries, readings, start, contexts, authors) in [
        ("zh-Hans", ZH, 3442, 915, 921, 1840),
        ("ja", JA, 638, 403, 409, 817),
    ] {
        let data = builtin(language).unwrap();
        assert_eq!(data.readings().len(), readings);
        assert_eq!(data.continuations().count(), contexts);
        assert_eq!(
            data.continuations().map(|(_, a)| a.len()).sum::<usize>(),
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
fn alternatives_inverse_rejects_changed_old_bytes_mutations_removals_and_unknown_future() {
    for (raw, new_author) in [
        (include_str!("../data/lexicons/en.json"), EN[0].3[0]),
        (include_str!("../data/lexicons/zh-Hans.json"), ZH[0].3[0]),
        (include_str!("../data/lexicons/ja.json"), JA[0].3[0]),
    ] {
        let historical = alternatives_resource::without_rescheduling(raw);
        let raw = historical.as_str();
        let changed_old = raw.replacen("\"language\": ", "\"language\":  ", 1);
        assert_ne!(changed_old, raw);
        assert!(
            std::panic::catch_unwind(|| alternatives_resource::without_alternatives(&changed_old))
                .is_err()
        );
        let quoted_author = serde_json::to_string(new_author).unwrap();
        if !raw.contains(&quoted_author) {
            continue; // The exact rephrasing resource is accepted before the RED append.
        }
        let changed = raw.replacen(
            &quoted_author,
            &serde_json::to_string("changed authored tail").unwrap(),
            1,
        );
        assert!(
            std::panic::catch_unwind(|| alternatives_resource::without_alternatives(&changed))
                .is_err()
        );
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        for change in 0..3 {
            let mut changed = value.clone();
            if value["language"] == "en" {
                let layer = &mut changed["word_layers"][63];
                match change {
                    0 => {
                        layer["next_words"].as_array_mut().unwrap().remove(9);
                    }
                    1 => {
                        layer["sentences"].as_array_mut().unwrap().remove(18);
                    }
                    _ => layer["next_words"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(["future context", ["future"]])),
                }
            } else {
                let start = if value["language"] == "ja" { 403 } else { 915 };
                let contexts = changed["continuations"].as_array_mut().unwrap();
                match change {
                    0 => {
                        contexts.remove(start);
                    }
                    1 => {
                        contexts[start][1].as_array_mut().unwrap().remove(0);
                    }
                    _ => contexts.push(serde_json::json!(["future context", ["future author"]])),
                }
            }
            let changed = serde_json::to_string_pretty(&changed).unwrap() + "\n";
            assert!(
                std::panic::catch_unwind(|| alternatives_resource::without_alternatives(&changed))
                    .is_err()
            );
        }
    }
}

#[test]
fn alternatives_keeps_all_6216_published_english_first_seen_ranks() {
    let previous =
        alternatives_resource::without_alternatives(include_str!("../data/lexicons/en.json"));
    let previous = Lexicon::from_json(&previous).unwrap();
    assert_eq!(previous.word_layers().len(), 64);
    let mut before_presence = BTreeSet::new();
    for layer in &previous.word_layers()[..63] {
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
            before_presence.insert(word.replace('’', "'").to_ascii_lowercase());
        }
    }
    for &(context, words, _, authors) in EN {
        for text in [context, words[0], words[1], authors[0], authors[1]] {
            for word in text
                .split(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '\'' | '’')))
                .filter(|word| !word.is_empty())
            {
                assert!(
                    before_presence.contains(&word.replace('’', "'").to_ascii_lowercase()),
                    "alternatives token was absent before presence: {word:?} in {text:?}"
                );
            }
        }
    }
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
                    .flat_map(|(_, words)| words.iter().map(String::as_str)),
            )
            .chain(layer.sentences.iter().flat_map(|s| {
                s.split(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '\'' | '’')))
                    .filter(|word| !word.is_empty())
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
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        hash,
        "edd8b14aa834036441481651ec3cd72d0a00e36c812756f5b292a726789fbd3a"
    );
}

#[test]
fn alternatives_english_six_branches_partial_full_and_literal_commit_once() {
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
fn alternatives_chinese_joined_separated_tones_uppercase_han_and_partials() {
    for (i, &(reading, partial, word, authors)) in ZH.iter().enumerate() {
        let tone = ["bei4'xuan3", "ti4'dai4", "shang1'liang5"][i];
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
            SUITE.choices("zh-Hans", &seed, word, &authors);
            for text in [word, authors[0], authors[1], &seed] {
                SUITE.commit("zh-Hans", &seed, text);
            }
        }
    }
}

#[test]
fn alternatives_japanese_romaji_kana_case_full_and_partial_with_literal_raw() {
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["すき", "かたほう", "きめる"][i];
        assert_eq!(romaji_to_hiragana(reading), kana);
        assert_eq!(romaji_to_hiragana(partial), ["すk", "かたほ", "きめr"][i]);
        for seed in [
            reading.into(),
            reading.to_ascii_uppercase(),
            kana.into(),
            partial.into(),
            partial.to_ascii_uppercase(),
            ["すk", "かたほ", "きめr"][i].into(),
            word.into(),
        ] {
            SUITE.choices("ja", &seed, word, &authors);
            for text in [word, authors[0], authors[1], &seed] {
                SUITE.commit("ja", &seed, text);
            }
            let ime = SUITE.engine("ja", &seed);
            assert!(ime.candidates().iter().any(
                |candidate| candidate.text == seed && candidate.kind == CandidateKind::Literal
            ));
        }
    }
}

#[test]
fn alternatives_six_horizontal_gaps_keep_finite_authored_choices() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = ["would", "you", "be", "happy", "with", "te"].join(gap);
        let word = seed.strip_suffix("te").unwrap().to_owned() + "tea";
        SUITE.choices(
            "en",
            &seed,
            &word,
            &[&(word.clone() + " instead of coffee?")],
        );
        SUITE.commit("en", &seed, &seed);
        for &(reading, _, word, authors) in ZH {
            let seed = reading.replace('\'', gap);
            SUITE.choices("zh-Hans", &seed, word, &authors);
            SUITE.commit("zh-Hans", &seed, &seed);
        }
        for (i, (a, b)) in [("su", "ki"), ("kata", "hou"), ("ki", "meru")]
            .into_iter()
            .enumerate()
        {
            let seed = format!("{a}{gap}{b}");
            SUITE.choices("ja", &seed, JA[i].2, &JA[i].3);
            SUITE.commit("ja", &seed, &seed);
        }
    }
}

#[test]
fn alternatives_adoption_and_long_english_draft_preserve_uncommitted_prefix() {
    for (language, entries) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(reading, _, word, authors) in entries {
            SUITE.adopt(language, reading, word, &authors);
        }
    }
    let prefix = "前面的文字🙂，保持原样。".repeat(35);
    let seed = format!("{prefix}if that does not work, we could tr");
    let word = format!("{prefix}if that does not work, we could try");
    let author = format!("{word} a different place.");
    SUITE.choices("en", &seed, &word, &[&author]);
    for text in [&*seed, &*word, &*author] {
        SUITE.commit("en", &seed, text);
    }
}

#[test]
fn alternatives_seven_line_breaks_and_opaque_tokens_keep_literal_raw_rescue() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, authors) in [
            (
                "en",
                format!("if that does not work,{line}we could tr"),
                EN[0].3,
            ),
            ("zh-Hans", format!("bei{line}xuan"), ZH[0].3),
            ("ja", format!("su{line}ki"), JA[0].3),
        ] {
            let ime = SUITE.engine(language, &seed);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| authors.contains(&candidate.text.as_str()))
            );
            SUITE.commit(language, &seed, &seed);
        }
    }
    for (language, seed) in [
        ("en", "if that does not work, we could field_tr"),
        (
            "en",
            "https://example.invalid/if_that_does_not_work_we_could_tr",
        ),
        ("zh-Hans", "user_bei_xuan_42"),
        ("ja", "user_suki_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
    for seed in [
        "if that does not work, we could trz",
        "if that does not work, we could waiz",
        "would you be happy with teq",
        "would you be happy with coffez",
        "we could go with the firz",
        "we could go with the seconz",
    ] {
        let ime = SUITE.engine("en", seed);
        assert!(!ime.candidates().iter().any(|candidate| {
            EN.iter()
                .any(|case| case.3.contains(&candidate.text.as_str()))
        }));
        SUITE.commit("en", seed, seed);
    }
}

#[test]
fn alternatives_synthetic_pending_disabled_errors_and_empty_models_keep_local_choices() {
    SUITE.missing_models(
        "en",
        "if that does not work, we could tr",
        "if that does not work, we could try",
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
                    "another option would be waitin",
                    "another option would be waiting",
                    "another option would be waiting until tomorrow.",
                ),
                (
                    "would you rather wai",
                    "would you rather wait",
                    "would you rather wait or come back later?",
                ),
                (
                    "i prefer the firs",
                    "i prefer the first",
                    "i prefer the first one.",
                ),
            ] {
                SUITE.choices(language, seed, word, &[author]);
                for text in [seed, word, author] {
                    SUITE.commit(language, seed, text);
                }
            }
        }
        "zh-Hans" => {
            let seed = "ji'cuo'le";
            let word = "记错了";
            let authors = [
                "记错了，刚才说的应该是明天下午。",
                "记错了，不是三点，是四点。",
            ];
            SUITE.choices(language, seed, word, &authors);
            for text in [seed, word, authors[0], authors[1]] {
                SUITE.commit(language, seed, text);
            }
        }
        "ja" => {
            let seed = "youyaku";
            let word = "要約";
            let authors = [
                "要約すると、先に時間を決めてから場所を選びます。",
                "要約すると、今日は予定を確認するだけです。",
            ];
            SUITE.choices(language, seed, word, &authors);
            for text in [seed, word, authors[0], authors[1]] {
                SUITE.commit(language, seed, text);
            }
        }
        _ => unreachable!(),
    }
}

#[test]
fn alternatives_previous_neighbor_controls_still_commit_once() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn alternatives_old_and_new_choices_coexist_with_private_8_8_9_packs() {
    const NAME: &str = "alternatives_old_and_new_choices_coexist_with_private_8_8_9_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_ALTERNATIVES_DIALOGUE_TEST_CHILD",
        old_controls,
        |language| {
            if language == "en" {
                for &(context, words, partials, authors) in EN {
                    for i in 0..2 {
                        let word = format!("{context} {}", words[i]);
                        for seed in [format!("{context} {}", partials[i]), word.clone()] {
                            SUITE.choices(language, &seed, &word, &[authors[i]]);
                            for text in [&*seed, &*word, authors[i]] {
                                SUITE.commit(language, &seed, text);
                            }
                        }
                    }
                }
            } else {
                for &(reading, partial, word, authors) in if language == "ja" { JA } else { ZH } {
                    for seed in [reading, partial, word] {
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

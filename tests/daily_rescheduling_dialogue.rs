//! Finite authored temporary plan changes and rescheduling, with unchanged codec boundaries.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/rescheduling_resource.rs"]
mod rescheduling_resource;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::{BTreeMap, BTreeSet};
use suzaku_map::{
    ime::candidate_mix::CandidateKind,
    languages::japanese::romaji_to_hiragana,
    lexicon::{Lexicon, builtin},
};

const SUITE: Suite = Suite {
    marker: "RESCHEDULING",
};
// These finite authored cases are independent of candidate output and resource parsing.
const EN: &[EnglishCase] = &[
    (
        "something came up, could we meet",
        ["tomorrow", "later"],
        ["tomorro", "late"],
        [
            "something came up, could we meet tomorrow at the same time?",
            "something came up, could we meet later this week?",
        ],
    ),
    (
        "if you are running late, we can",
        ["wait", "start"],
        ["wai", "star"],
        [
            "if you are running late, we can wait here for a few minutes.",
            "if you are running late, we can start a little later.",
        ],
    ),
    (
        "let's keep the same place and change the",
        ["time", "day"],
        ["tim", "da"],
        [
            "let's keep the same place and change the time to three instead.",
            "let's keep the same place and change the day to friday.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "lin'shi'you'an'pai",
        "lin'shi'you'an'pa",
        "临时有安排",
        [
            "临时有安排，今天可能没法过去了。",
            "临时有安排，我们能换个时间吗？",
        ],
    ),
    (
        "gai'ge'shi'jian'ba",
        "gai'ge'shi'jian'b",
        "改个时间吧",
        [
            "改个时间吧，明天下午你方便吗？",
            "改个时间吧，我今天临时有事。",
        ],
    ),
    (
        "gai'yue'shi'jian",
        "gai'yue'shi'jia",
        "改约时间",
        [
            "改约时间确定后，麻烦再发我一次。",
            "改约时间先别定，我再确认一下明天的安排。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "raishuu",
        "raishu",
        "来週",
        [
            "来週なら、今のところ火曜日が空いています。",
            "来週に変更しても大丈夫ですか。",
        ],
    ),
    (
        "nittei",
        "nitte",
        "日程",
        [
            "日程を調整したいので、都合のよい日を教えてください。",
            "日程が変わったら、早めに連絡します。",
        ],
    ),
    (
        "chousei",
        "chouse",
        "調整",
        [
            "調整が必要なら、別の時間も考えましょう。",
            "調整できたら、改めて連絡します。",
        ],
    ),
];

#[test]
fn rescheduling_appends_nine_contexts_eighteen_authors_with_exact_alternatives_inverse() {
    for (raw, previous_sha) in [
        (
            include_str!("../data/lexicons/en.json"),
            "e48baa0c2b12dc3b024082834f5bc452f0afa201fda68cf290a66057b89b58ac",
        ),
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            "093e752b7d626d21f46342ba7446a238cf3a0b8ac934f469564f0da6d46bb4ae",
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            "b53c371d6fe37ce983f72c9cdd92c975173e746c12021219c40b48a27400fc19",
        ),
    ] {
        let previous = rescheduling_resource::without_rescheduling(raw);
        // The shared guard checks the complete returned bytes, with no projection.
        SUITE.preserve_resource(
            &previous,
            "absent-rescheduling-marker",
            "absent-end",
            previous_sha,
        );
        assert_eq!(
            rescheduling_resource::without_rescheduling(&previous),
            previous
        );
    }
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 64);
    assert_eq!(en.next_words().count(), 968);
    assert_eq!(en.sentences().count(), 1900);
    let layer = &en.word_layers()[63];
    assert_eq!(layer.id, "daily_presence_dialogue");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 53);
    assert_eq!(layer.sentences.len(), 106);
    assert_eq!(layer.next_words[12..15].len(), EN.len());
    assert_eq!(layer.sentences[24..30].len(), EN.len() * 2);
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[12 + i].0, context);
        assert_eq!(layer.next_words[12 + i].1, words);
        assert_eq!(&layer.sentences[24 + 2 * i..26 + 2 * i], authors);
    }
    for (language, entries, readings, start, contexts, authors) in [
        ("zh-Hans", ZH, 3442, 918, 959, 1916),
        ("ja", JA, 638, 406, 447, 893),
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
fn rescheduling_inverse_rejects_changed_old_bytes_mutations_removals_and_unknown_future() {
    for (raw, new_author) in [
        (include_str!("../data/lexicons/en.json"), EN[0].3[0]),
        (include_str!("../data/lexicons/zh-Hans.json"), ZH[0].3[0]),
        (include_str!("../data/lexicons/ja.json"), JA[0].3[0]),
    ] {
        let changed_old = raw.replacen("\"language\": ", "\"language\":  ", 1);
        assert_ne!(changed_old, raw);
        assert!(
            std::panic::catch_unwind(|| rescheduling_resource::without_rescheduling(&changed_old))
                .is_err()
        );
        let quoted_author = serde_json::to_string(new_author).unwrap();
        if !raw.contains(&quoted_author) {
            continue; // The exact alternatives resource is accepted before the RED append.
        }
        let changed = raw.replacen(
            &quoted_author,
            &serde_json::to_string("changed authored tail").unwrap(),
            1,
        );
        assert!(
            std::panic::catch_unwind(|| rescheduling_resource::without_rescheduling(&changed))
                .is_err()
        );
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        for change in 0..3 {
            let mut changed = value.clone();
            if value["language"] == "en" {
                let layer = &mut changed["word_layers"][63];
                match change {
                    0 => {
                        layer["next_words"].as_array_mut().unwrap().remove(12);
                    }
                    1 => {
                        layer["sentences"].as_array_mut().unwrap().remove(24);
                    }
                    _ => layer["next_words"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(["future context", ["future"]])),
                }
            } else {
                let start = if value["language"] == "ja" { 406 } else { 918 };
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
                std::panic::catch_unwind(|| rescheduling_resource::without_rescheduling(&changed))
                    .is_err()
            );
        }
    }
}

#[test]
fn rescheduling_keeps_all_6216_published_english_first_seen_ranks() {
    let previous =
        rescheduling_resource::without_rescheduling(include_str!("../data/lexicons/en.json"));
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
                    "rescheduling token was absent before presence: {word:?} in {text:?}"
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
fn rescheduling_english_six_branches_partial_full_and_literal_commit_once() {
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
fn rescheduling_chinese_joined_separated_tones_uppercase_han_and_partials() {
    for (i, &(reading, partial, word, authors)) in ZH.iter().enumerate() {
        let tone = [
            "lin2'shi2'you3'an1'pai2",
            "gai3'ge4'shi2'jian1'ba5",
            "gai3'yue1'shi2'jian1",
        ][i];
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
fn rescheduling_japanese_romaji_kana_case_full_and_partial_with_literal_raw() {
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["らいしゅう", "にってい", "ちょうせい"][i];
        assert_eq!(romaji_to_hiragana(reading), kana);
        assert_eq!(
            romaji_to_hiragana(partial),
            ["らいしゅ", "にって", "ちょうせ"][i]
        );
        for seed in [
            reading.into(),
            reading.to_ascii_uppercase(),
            kana.into(),
            partial.into(),
            partial.to_ascii_uppercase(),
            ["らいしゅ", "にって", "ちょうせ"][i].into(),
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
fn rescheduling_six_horizontal_gaps_keep_finite_authored_choices() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = ["something", "came", "up,", "could", "we", "meet", "tomorro"].join(gap);
        let word = seed.strip_suffix("tomorro").unwrap().to_owned() + "tomorrow";
        SUITE.choices(
            "en",
            &seed,
            &word,
            &[&(word.clone() + " at the same time?")],
        );
        SUITE.commit("en", &seed, &seed);
        for &(reading, _, word, authors) in ZH {
            let seed = reading.replace('\'', gap);
            SUITE.choices("zh-Hans", &seed, word, &authors);
            SUITE.commit("zh-Hans", &seed, &seed);
        }
        for (i, (a, b)) in [("rai", "shuu"), ("ni", "ttei"), ("chou", "sei")]
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
fn rescheduling_adoption_and_long_english_draft_preserve_uncommitted_prefix() {
    for (language, entries) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(reading, _, word, authors) in entries {
            SUITE.adopt(language, reading, word, &authors);
        }
    }
    let prefix = "前面的文字🙂，保持原样。".repeat(35);
    let seed = format!("{prefix}something came up, could we meet tomorro");
    let word = format!("{prefix}something came up, could we meet tomorrow");
    let author = format!("{word} at the same time?");
    SUITE.choices("en", &seed, &word, &[&author]);
    for text in [&*seed, &*word, &*author] {
        SUITE.commit("en", &seed, text);
    }
}

#[test]
fn rescheduling_seven_line_breaks_and_opaque_tokens_keep_literal_raw_rescue() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, authors) in [
            (
                "en",
                format!("something came up,{line}could we meet tomorro"),
                EN[0].3,
            ),
            ("zh-Hans", format!("lin{line}shi you an pai"), ZH[0].3),
            ("ja", format!("rai{line}shuu"), JA[0].3),
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
        ("en", "something came up, could we meet field_tomorro"),
        (
            "en",
            "https://example.invalid/something_came_up_could_we_meet_tomorro",
        ),
        ("zh-Hans", "user_lin_shi_you_an_pai_42"),
        ("ja", "user_raishuu_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
    for seed in [
        "something came up, could we meet tomorrz",
        "something came up, could we meet latez",
        "if you are running late, we can waiz",
        "if you are running late, we can starz",
        "let's keep the same place and change the timz",
        "let's keep the same place and change the dayz",
    ] {
        let ime = SUITE.engine("en", seed);
        assert!(!ime.candidates().iter().any(|candidate| {
            EN.iter().any(|&(context, words, _, authors)| {
                authors.contains(&candidate.text.as_str())
                    || words
                        .iter()
                        .any(|word| candidate.text == format!("{context} {word}"))
            })
        }));
        SUITE.commit("en", seed, seed);
    }
}

#[test]
fn rescheduling_synthetic_pending_disabled_errors_and_empty_models_keep_local_choices() {
    SUITE.missing_models(
        "en",
        "something came up, could we meet tomorro",
        "something came up, could we meet tomorrow",
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
                    "can we move this to tomorro",
                    "can we move this to tomorrow",
                    "can we move this to tomorrow morning?",
                ),
                (
                    "could we meet a little late",
                    "could we meet a little later",
                    "could we meet a little later this afternoon?",
                ),
                (
                    "let me know if the time change",
                    "let me know if the time changes",
                    "let me know if the time changes.",
                ),
            ] {
                SUITE.choices(language, seed, word, &[author]);
                for text in [seed, word, author] {
                    SUITE.commit(language, seed, text);
                }
            }
        }
        "zh-Hans" => {
            let seed = "bei'xuan";
            let word = "备选";
            let authors = [
                "备选方案先留着，等大家确认后再决定。",
                "备选的时间也发给我吧。",
            ];
            SUITE.choices(language, seed, word, &authors);
            for text in [seed, word, authors[0], authors[1]] {
                SUITE.commit(language, seed, text);
            }
        }
        "ja" => {
            let seed = "kimeru";
            let word = "決める";
            let authors = [
                "決める前に、みんなの希望も聞きたいです。",
                "決めるのは明日でも大丈夫です。",
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
fn rescheduling_previous_neighbor_controls_still_commit_once() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn rescheduling_old_and_new_choices_coexist_with_private_8_8_9_packs() {
    const NAME: &str = "rescheduling_old_and_new_choices_coexist_with_private_8_8_9_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_RESCHEDULING_DIALOGUE_TEST_CHILD",
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

//! Finite authored rephrasings and clarification turns, with unchanged codec boundaries.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/rephrasing_resource.rs"]
mod rephrasing_resource;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::{BTreeMap, BTreeSet};
use suzaku_map::{
    ime::candidate_mix::CandidateKind,
    languages::japanese::romaji_to_hiragana,
    lexicon::{Lexicon, builtin},
};

const SUITE: Suite = Suite {
    marker: "REPHRASING",
};
// These finite authored cases are independent of candidate output and resource parsing.
const EN: &[EnglishCase] = &[
    (
        "actually, i meant",
        ["tomorrow", "tonight"],
        ["tomorro", "tonigh"],
        [
            "actually, i meant tomorrow morning, not today.",
            "actually, i meant tonight after dinner, not tomorrow.",
        ],
    ),
    (
        "let me put that a little more",
        ["clearly", "simply"],
        ["clearl", "simpl"],
        [
            "let me put that a little more clearly: we can start after lunch.",
            "let me put that a little more simply: we should wait here.",
        ],
    ),
    (
        "could we check the",
        ["details", "times"],
        ["detail", "time"],
        [
            "could we check the details before we leave?",
            "could we check the times again before we make plans?",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "ji'cuo'le",
        "ji'cuo'l",
        "记错了",
        [
            "记错了，刚才说的应该是明天下午。",
            "记错了，不是三点，是四点。",
        ],
    ),
    (
        "huan'yi'zhong'shuo'fa",
        "huan'yi'zhong'shuo'f",
        "换一种说法",
        [
            "换一种说法，我是想先听听大家的意见。",
            "换一种说法，我们先把时间定下来，再选地方。",
        ],
    ),
    (
        "zan'bu'que'ding",
        "zan'bu'que'din",
        "暂不确定",
        [
            "暂不确定，我想先核对一下再回复你。",
            "暂不确定能不能赶上，晚点给你准信。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "youyaku",
        "youyak",
        "要約",
        [
            "要約すると、先に時間を決めてから場所を選びます。",
            "要約すると、今日は予定を確認するだけです。",
        ],
    ),
    (
        "shousai",
        "shousa",
        "詳細",
        [
            "詳細はまだ分からないので、確認してからお伝えします。",
            "詳細がはっきりするまでは、返事を少し待ってください。",
        ],
    ),
    (
        "jijitsu",
        "jijits",
        "事実",
        [
            "事実と違っていたので、先ほどの説明を訂正します。",
            "事実を確認したところ、会う日は明日でした。",
        ],
    ),
];

#[test]
fn rephrasing_appends_nine_contexts_eighteen_authors_with_exact_turntaking_inverse() {
    for (raw, previous_sha) in [
        (
            include_str!("../data/lexicons/en.json"),
            "aef47b5fdab0f0af87dc1773f2fdc295b25412083157925967bbcbb78b367677",
        ),
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            "44c141800c25846b4038d9361408648849cb68eb6c574df1fc1e50dd255b8efd",
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            "b8deee7e84006ecbd0b55436c2ac9be55ccc93bbee6f731f634f2c7fbfc61b1e",
        ),
    ] {
        let previous = rephrasing_resource::without_rephrasing(raw);
        // The shared guard checks the complete returned bytes, with no projection.
        SUITE.preserve_resource(
            &previous,
            "absent-rephrasing-marker",
            "absent-end",
            previous_sha,
        );
        assert_eq!(rephrasing_resource::without_rephrasing(&previous), previous);
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
    assert_eq!(layer.next_words[6..9].len(), EN.len());
    assert_eq!(layer.sentences[12..18].len(), EN.len() * 2);
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[6 + i].0, context);
        assert_eq!(layer.next_words[6 + i].1, words);
        assert_eq!(&layer.sentences[12 + 2 * i..14 + 2 * i], authors);
    }
    for (language, entries, readings, start, contexts, authors) in [
        ("zh-Hans", ZH, 3442, 912, 959, 1916),
        ("ja", JA, 638, 400, 447, 893),
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
fn rephrasing_inverse_rejects_changed_old_bytes_mutations_removals_and_unknown_future() {
    for (raw, new_author) in [
        (include_str!("../data/lexicons/en.json"), EN[0].3[0]),
        (include_str!("../data/lexicons/zh-Hans.json"), ZH[0].3[0]),
        (include_str!("../data/lexicons/ja.json"), JA[0].3[0]),
    ] {
        let historical = rephrasing_resource::without_alternatives(raw);
        let raw = historical.as_str();
        let changed_old = raw.replacen("\"language\": ", "\"language\":  ", 1);
        assert_ne!(changed_old, raw);
        assert!(
            std::panic::catch_unwind(|| rephrasing_resource::without_rephrasing(&changed_old))
                .is_err()
        );
        if !raw.contains(new_author) {
            continue; // The exact turntaking resource is accepted before the RED append.
        }
        let changed = raw.replacen(new_author, "changed authored tail", 1);
        assert!(
            std::panic::catch_unwind(|| rephrasing_resource::without_rephrasing(&changed)).is_err()
        );
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        for change in 0..3 {
            let mut changed = value.clone();
            if value["language"] == "en" {
                let layer = &mut changed["word_layers"][63];
                match change {
                    0 => {
                        layer["next_words"].as_array_mut().unwrap().remove(6);
                    }
                    1 => {
                        layer["sentences"].as_array_mut().unwrap().remove(12);
                    }
                    _ => layer["next_words"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(["future context", ["future"]])),
                }
            } else {
                let start = if value["language"] == "ja" { 400 } else { 912 };
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
                std::panic::catch_unwind(|| rephrasing_resource::without_rephrasing(&changed))
                    .is_err()
            );
        }
    }
}

#[test]
fn rephrasing_keeps_all_6216_published_english_first_seen_ranks() {
    let previous =
        rephrasing_resource::without_rephrasing(include_str!("../data/lexicons/en.json"));
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
                    "rephrasing token was absent before presence: {word:?} in {text:?}"
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
fn rephrasing_english_six_branches_partial_full_and_literal_commit_once() {
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
fn rephrasing_chinese_joined_separated_tones_uppercase_han_and_partials() {
    for (i, &(reading, partial, word, authors)) in ZH.iter().enumerate() {
        let tone = [
            "ji4'cuo4'le5",
            "huan4'yi1'zhong3'shuo1'fa3",
            "zan4'bu4'que4'ding4",
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
fn rephrasing_japanese_romaji_kana_case_full_and_partial_with_literal_raw() {
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        let kana = ["ようやく", "しょうさい", "じじつ"][i];
        assert_eq!(romaji_to_hiragana(reading), kana);
        assert_eq!(
            romaji_to_hiragana(partial),
            ["ようやk", "しょうさ", "じじts"][i]
        );
        for seed in [
            reading.into(),
            reading.to_ascii_uppercase(),
            kana.into(),
            partial.into(),
            partial.to_ascii_uppercase(),
            ["ようやk", "しょうさ", "じじts"][i].into(),
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
fn rephrasing_six_horizontal_gaps_keep_finite_authored_choices() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = ["actually,", "i", "meant", "tomorro"].join(gap);
        let word = seed.strip_suffix("tomorro").unwrap().to_owned() + "tomorrow";
        SUITE.choices(
            "en",
            &seed,
            &word,
            &[&(word.clone() + " morning, not today.")],
        );
        SUITE.commit("en", &seed, &seed);
        for &(reading, _, word, authors) in ZH {
            let seed = reading.replace('\'', gap);
            SUITE.choices("zh-Hans", &seed, word, &authors);
            SUITE.commit("zh-Hans", &seed, &seed);
        }
        for (i, (a, b)) in [("you", "yaku"), ("shou", "sai"), ("ji", "jitsu")]
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
fn rephrasing_adoption_and_long_english_draft_preserve_uncommitted_prefix() {
    for (language, entries) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(reading, _, word, authors) in entries {
            SUITE.adopt(language, reading, word, &authors);
        }
    }
    let prefix = "前面的文字🙂，保持原样。".repeat(35);
    let seed = format!("{prefix}actually, i meant tomorro");
    let word = format!("{prefix}actually, i meant tomorrow");
    let author = format!("{word} morning, not today.");
    SUITE.choices("en", &seed, &word, &[&author]);
    for text in [&*seed, &*word, &*author] {
        SUITE.commit("en", &seed, text);
    }
}

#[test]
fn rephrasing_seven_line_breaks_and_opaque_tokens_keep_literal_raw_rescue() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, authors) in [
            ("en", format!("actually, i{line}meant tomorro"), EN[0].3),
            ("zh-Hans", format!("ji{line}cuo le"), ZH[0].3),
            ("ja", format!("you{line}yaku"), JA[0].3),
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
        ("en", "actually, i meant field_tomorro"),
        ("en", "https://example.invalid/actually_i_meant_tomorro"),
        ("zh-Hans", "user_ji_cuo_le_42"),
        ("ja", "user_youyaku_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
    for seed in [
        "actually, i mean tomorro",
        "let me put that a little clearl",
        "could we check our detail",
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
fn rephrasing_synthetic_pending_disabled_errors_and_empty_models_keep_local_choices() {
    SUITE.missing_models(
        "en",
        "actually, i meant tomorro",
        "actually, i meant tomorrow",
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
                    "let me check the det",
                    "let me check the details",
                    "let me check the details.",
                ),
                (
                    "are you ready to orde",
                    "are you ready to order",
                    "are you ready to order or do you need another minute?",
                ),
            ] {
                SUITE.choices(language, seed, word, &[author]);
                for text in [seed, word, author] {
                    SUITE.commit(language, seed, text);
                }
            }
        }
        "zh-Hans" => {
            let seed = "fa'cuo'le";
            let word = "发错了";
            let authors = ["发错了，不好意思。", "发错了，我重新发给你。"];
            SUITE.choices(language, seed, word, &authors);
            for text in [seed, word, authors[0], authors[1]] {
                SUITE.commit(language, seed, text);
            }
        }
        "ja" => {
            let seed = "hosoku";
            let word = "補足";
            let authors = [
                "補足があれば教えてください。",
                "補足してもらえると助かります。",
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
fn rephrasing_previous_neighbor_controls_still_commit_once() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn rephrasing_old_and_new_choices_coexist_with_private_8_8_9_packs() {
    const NAME: &str = "rephrasing_old_and_new_choices_coexist_with_private_8_8_9_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_REPHRASING_DIALOGUE_TEST_CHILD",
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

//! Finite arrival check-ins and reassurance, not live safety tracking or trip execution.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/reassurance_resource.rs"]
mod reassurance_resource;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::BTreeMap;
use suzaku_map::{
    ime::candidate_mix::CandidateKind,
    languages::japanese::romaji_to_hiragana,
    lexicon::{Lexicon, WordLayer, builtin},
};

const SUITE: Suite = Suite {
    marker: "REASSURANCE",
};
// Authored expectations are fixed independently of resource parsing and candidate output.
const EN: &[EnglishCase] = &[
    (
        "when you get home, send me a",
        ["message", "text"],
        ["messag", "tex"],
        [
            "when you get home, send me a message so i know you're safe.",
            "when you get home, send me a text when you have a moment.",
        ],
    ),
    (
        "did you make it",
        ["home", "back"],
        ["hom", "bac"],
        [
            "did you make it home safely after the long trip?",
            "did you make it back okay, or do you need a hand?",
        ],
    ),
    (
        "just letting you know we're",
        ["home", "safe"],
        ["hom", "saf"],
        [
            "just letting you know we're home and the trip went well.",
            "just letting you know we're safe and everyone is fine.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "bie'dan'xin",
        "bie'dan'xi",
        "别担心",
        [
            "别担心，我们可以一件一件慢慢处理。",
            "别担心，有需要的话我可以陪你一起去。",
        ],
    ),
    (
        "man'man'lai",
        "man'man'la",
        "慢慢来",
        [
            "慢慢来，不用一次把所有事情做完。",
            "慢慢来，等你准备好了我们再继续。",
        ],
    ),
    (
        "zhu'yi'shen'ti",
        "zhu'yi'shen't",
        "注意身体",
        [
            "注意身体，别把自己弄得太累。",
            "注意身体，忙的时候也记得按时吃饭。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "ogenkidesuka",
        "ogenkidesuk",
        "お元気ですか",
        [
            "お元気ですか。最近、無理をしていませんか。",
            "お元気ですか。返事は落ち着いたときで大丈夫です。",
        ],
    ),
    (
        "ohisashiburi",
        "ohisashibur",
        "お久しぶり",
        [
            "お久しぶりです。最近は元気にしていますか。",
            "お久しぶりです。また近況を聞けたらうれしいです。",
        ],
    ),
    (
        "inemuri",
        "inemur",
        "居眠り",
        [
            "居眠りしそうなので、少し休んでからまた連絡します。",
            "居眠りしてしまって、返事が遅くなりました。もう起きています。",
        ],
    ),
];
const TONES: [&str; 3] = ["bie2'dan1'xin1", "man4'man4'lai2", "zhu4'yi4'shen1'ti3"];
const KANA: [&str; 3] = ["おげんきですか", "おひさしぶり", "いねむり"];
const PARTIAL_KANA: [&str; 3] = ["おげんきですk", "おひさしぶr", "いねむr"];
const PURE_KANA_NEAR: [&str; 3] = ["おげんきです", "おひさしぶ", "いねむ"];
const RESOURCES: &[(&str, &str)] = &[
    (
        include_str!("../data/lexicons/en.json"),
        "1522f322f693db7d18181221b3312a1ab8b905e2d48c1fbf794626acd3f2faf7",
    ),
    (
        include_str!("../data/lexicons/zh-Hans.json"),
        "6b2963727cc976b92a8a66ee989fcc95652a8e5a5ff3203e5de01f547bb4624b",
    ),
    (
        include_str!("../data/lexicons/ja.json"),
        "15ed727f43d55b79794409323a21bfddef900db010a4e67ff7e82b0d5549c827",
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
fn reassurance_appends_nine_contexts_eighteen_authors_with_exact_farewell_inverse() {
    for &(raw, expected_sha) in RESOURCES {
        let previous = reassurance_resource::without_reassurance(raw);
        SUITE.preserve_resource(&previous, "absent-reassurance", "absent-end", expected_sha);
        assert_eq!(
            reassurance_resource::without_reassurance(&previous),
            previous
        );
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
        assert_eq!(layer.next_words[21 + i].0, context);
        assert_eq!(layer.next_words[21 + i].1, words);
        assert_eq!(&layer.sentences[42 + 2 * i..44 + 2 * i], authors);
    }
    for (language, entries, readings, start, contexts, authors) in [
        ("zh-Hans", ZH, 3442, 927, 937, 1872),
        ("ja", JA, 638, 415, 425, 849),
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
fn reassurance_inverse_rejects_changed_old_bytes_mutations_removals_and_unknown_future() {
    for (&(raw, _), new_author) in RESOURCES.iter().zip([EN[0].3[0], ZH[0].3[0], JA[0].3[0]]) {
        let changed = raw.replacen("\"language\": ", "\"language\":  ", 1);
        assert_ne!(changed, raw);
        assert!(
            std::panic::catch_unwind(|| reassurance_resource::without_reassurance(&changed))
                .is_err()
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
            std::panic::catch_unwind(|| reassurance_resource::without_reassurance(&changed))
                .is_err()
        );
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        for mutation in 0..3 {
            let mut changed = value.clone();
            if value["language"] == "en" {
                let layer = &mut changed["word_layers"][63];
                match mutation {
                    0 => {
                        layer["next_words"].as_array_mut().unwrap().remove(21);
                    }
                    1 => {
                        layer["sentences"].as_array_mut().unwrap().remove(42);
                    }
                    _ => layer["next_words"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(["future context", ["future"]])),
                }
            } else {
                let start = if value["language"] == "ja" { 415 } else { 927 };
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
                std::panic::catch_unwind(|| reassurance_resource::without_reassurance(&changed))
                    .is_err()
            );
        }
    }
}

#[test]
fn reassurance_keeps_all_6216_published_english_first_seen_ranks() {
    let previous = reassurance_resource::without_reassurance(RESOURCES[0].0);
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
fn reassurance_english_six_branches_partial_full_and_literal_commit_once() {
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
fn reassurance_chinese_joined_separated_tones_uppercase_han_and_partials() {
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
fn reassurance_japanese_romaji_kana_case_full_and_partial_with_literal_raw() {
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
            PURE_KANA_NEAR[i].into(),
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
fn reassurance_six_horizontal_gaps_keep_finite_authored_choices() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = ["when", "you", "get", "home,", "send", "me", "a", "messag"].join(gap);
        let word = seed.strip_suffix("messag").unwrap().to_owned() + "message";
        probe(
            "en",
            &seed,
            &word,
            &[&(word.clone() + " so i know you're safe.")],
        );
        for &(reading, _, word, authors) in ZH {
            probe("zh-Hans", &reading.replace('\'', gap), word, &authors);
        }
        for (i, (a, b)) in [("ogenki", "desuka"), ("ohisashi", "buri"), ("inemu", "ri")]
            .into_iter()
            .enumerate()
        {
            probe("ja", &format!("{a}{gap}{b}"), JA[i].2, &JA[i].3);
        }
    }
}

#[test]
fn reassurance_adoption_and_long_english_draft_preserve_uncommitted_prefix() {
    for (language, entries) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(reading, _, word, authors) in entries {
            SUITE.adopt(language, reading, word, &authors);
        }
    }
    let prefix = "前面的文字🙂，保持原样。".repeat(35);
    let seed = format!("{prefix}when you get home, send me a messag");
    let word = format!("{prefix}when you get home, send me a message");
    probe(
        "en",
        &seed,
        &word,
        &[&(word.clone() + " so i know you're safe.")],
    );
}

#[test]
fn reassurance_seven_line_breaks_and_opaque_tokens_keep_literal_raw_rescue() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, authors) in [
            (
                "en",
                format!("when you get home,{line}send me a messag"),
                EN[0].3,
            ),
            ("zh-Hans", format!("bie{line}dan xin"), ZH[0].3),
            ("ja", format!("ogenki{line}desuka"), JA[0].3),
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
        ("en", "when you get home, send me a field_messag"),
        (
            "en",
            "https://example.invalid/when_you_get_home_send_me_a_messag",
        ),
        ("zh-Hans", "user_bie_dan_xin_42"),
        ("ja", "user_ogenkidesuka_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
    for seed in [
        "when you get home, send me a messagz",
        "when you get home, send me a texz",
        "did you make it homz",
        "did you make it bacz",
        "just letting you know we're homz",
        "just letting you know we're safz",
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
fn reassurance_synthetic_pending_disabled_errors_and_empty_models_keep_local_choices() {
    SUITE.missing_models(
        "en",
        "when you get home, send me a messag",
        "when you get home, send me a message",
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
                    "i should let you get back to wor",
                    "i should let you get back to work",
                    "i should let you get back to work now, thanks for the chat.",
                ),
                (
                    "i have to go now, but let's kee",
                    "i have to go now, but let's keep",
                    "i have to go now, but let's keep in touch.",
                ),
                (
                    "thanks for the chat, have a saf",
                    "thanks for the chat, have a safe",
                    "thanks for the chat, have a safe trip home.",
                ),
            ] {
                probe(language, seed, word, &[author]);
            }
        }
        "zh-Hans" => probe(
            language,
            "xian'zhe'yang'ba",
            "先这样吧",
            &[
                "先这样吧，今天就聊到这里。",
                "先这样吧，你早点休息，我们改天再聊。",
            ],
        ),
        "ja" => probe(
            language,
            "ojamashimashita",
            "お邪魔しました",
            &[
                "お邪魔しました。今日はゆっくりお話しできて楽しかったです。",
                "お邪魔しました。またお会いできるのを楽しみにしています。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn reassurance_previous_neighbor_controls_still_commit_once() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn reassurance_old_and_new_choices_coexist_with_private_8_8_9_packs() {
    const NAME: &str = "reassurance_old_and_new_choices_coexist_with_private_8_8_9_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_REASSURANCE_DIALOGUE_TEST_CHILD",
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

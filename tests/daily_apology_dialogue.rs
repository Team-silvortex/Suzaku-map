//! Finite late-reply explanations, corrections and resending, not message delivery or live actions.
#[path = "support/apology_resource.rs"]
mod apology_resource;
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::BTreeMap;
use suzaku_map::{
    ime::candidate_mix::CandidateKind,
    languages::japanese::romaji_to_hiragana,
    lexicon::{Lexicon, WordLayer, builtin},
};

const SUITE: Suite = Suite { marker: "APOLOGY" };
// Authored expectations are fixed independently of resource parsing and candidate output.
const EN: &[EnglishCase] = &[
    (
        "sorry for the late reply, i was",
        ["busy", "away"],
        ["bus", "awa"],
        [
            "sorry for the late reply, i was busy with work and missed your message.",
            "sorry for the late reply, i was away from my phone all afternoon.",
        ],
    ),
    (
        "thanks for pointing that out, i'll",
        ["check", "fix"],
        ["chec", "fi"],
        [
            "thanks for pointing that out, i'll check the details again.",
            "thanks for pointing that out, i'll fix that and send it again.",
        ],
    ),
    (
        "that was my fault, i'll",
        ["correct", "explain"],
        ["correc", "explai"],
        [
            "that was my fault, i'll correct the time and send an update.",
            "that was my fault, i'll explain what happened so it is clear.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "lou'kan'xiao'xi",
        "lou'kan'xiao'x",
        "漏看消息",
        [
            "漏看消息了，不好意思，我现在就回复你。",
            "漏看消息是我的疏忽，下次我会留意。",
        ],
    ),
    (
        "kan'cuo'le",
        "kan'cuo'l",
        "看错了",
        [
            "看错了，集合时间是明天下午，不是今天。",
            "看错了，刚才那条消息不是发给我的。",
        ],
    ),
    (
        "bao'qian'rang'ni'deng'le",
        "bao'qian'rang'ni'deng'l",
        "抱歉让你等了",
        [
            "抱歉让你等了，刚才没有看到手机上的消息。",
            "抱歉让你等了，我刚忙完，现在可以慢慢聊。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "shazai",
        "shaza",
        "謝罪",
        [
            "謝罪します。大事な連絡を見落としていました。",
            "謝罪します。先ほどは私の説明が間違っていました。",
        ],
    ),
    (
        "saisou",
        "saiso",
        "再送",
        [
            "再送します。さっきのメッセージが届いていなかったようです。",
            "再送していただけますか。添付の写真が見当たりません。",
        ],
    ),
    (
        "oshirase",
        "oshiras",
        "お知らせ",
        [
            "お知らせが遅くなりました。明日の集合場所は駅の東口です。",
            "お知らせに一つ訂正があります。開始時間は十時ではなく、十一時です。",
        ],
    ),
];
const TONES: [&str; 3] = [
    "lou4'kan4'xiao1'xi5",
    "kan4'cuo4'le5",
    "bao4'qian4'rang4'ni3'deng3'le5",
];
const KANA: [&str; 3] = ["しゃざい", "さいそう", "おしらせ"];
const PARTIAL_KANA: [&str; 3] = ["しゃざ", "さいそ", "おしらs"];
const PURE_KANA_NEAR: [&str; 3] = ["しゃざ", "さいそ", "おしら"];
const RESOURCES: &[(&str, &str)] = &[
    (
        include_str!("../data/lexicons/en.json"),
        "19e94be2d721f78e6740e9d6841ffe451a00d69a25631ef29716c61a3be7ee34",
    ),
    (
        include_str!("../data/lexicons/zh-Hans.json"),
        "b117bd7ff8fd4c7e5f044131db0c60e9e95fa240210f53333a3fa228bbad48e7",
    ),
    (
        include_str!("../data/lexicons/ja.json"),
        "8ca1500cb7ff53dd27c542f83587e4022d611c45fdfb7253145b158e55a113a4",
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
fn apology_appends_nine_contexts_eighteen_authors_with_exact_reassurance_inverse() {
    for &(raw, expected_sha) in RESOURCES {
        let previous = apology_resource::without_apology(raw);
        SUITE.preserve_resource(&previous, "absent-apology", "absent-end", expected_sha);
        assert_eq!(apology_resource::without_apology(&previous), previous);
    }
    let en = builtin("en").unwrap();
    assert_eq!(
        (
            en.word_layers().len(),
            en.next_words().count(),
            en.sentences().count()
        ),
        (64, 968, 1900)
    );
    let layer = &en.word_layers()[63];
    assert_eq!(layer.id, "daily_presence_dialogue");
    assert!(layer.words.is_empty());
    assert_eq!((layer.next_words.len(), layer.sentences.len()), (53, 106));
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[24 + i].0, context);
        assert_eq!(layer.next_words[24 + i].1, words);
        assert_eq!(&layer.sentences[48 + 2 * i..50 + 2 * i], authors);
    }
    for (language, entries, readings, start, contexts, authors) in [
        ("zh-Hans", ZH, 3442, 930, 959, 1916),
        ("ja", JA, 638, 418, 447, 893),
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
fn apology_inverse_rejects_changed_old_bytes_mutations_removals_and_unknown_future() {
    for (&(raw, _), new_author) in RESOURCES.iter().zip([EN[0].3[0], ZH[0].3[0], JA[0].3[0]]) {
        let changed = raw.replacen("\"language\": ", "\"language\":  ", 1);
        assert_ne!(changed, raw);
        assert!(std::panic::catch_unwind(|| apology_resource::without_apology(&changed)).is_err());
        let quoted = serde_json::to_string(new_author).unwrap();
        if !raw.contains(&quoted) {
            continue; // Exact previous resources remain valid before the RED append.
        }
        let changed = raw.replacen(
            &quoted,
            &serde_json::to_string("changed authored tail").unwrap(),
            1,
        );
        assert!(std::panic::catch_unwind(|| apology_resource::without_apology(&changed)).is_err());
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        for mutation in 0..3 {
            let mut changed = value.clone();
            if value["language"] == "en" {
                let layer = &mut changed["word_layers"][63];
                match mutation {
                    0 => {
                        layer["next_words"].as_array_mut().unwrap().remove(24);
                    }
                    1 => {
                        layer["sentences"].as_array_mut().unwrap().remove(48);
                    }
                    _ => layer["next_words"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(["future context", ["future"]])),
                }
            } else {
                let start = if value["language"] == "ja" { 418 } else { 930 };
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
                std::panic::catch_unwind(|| apology_resource::without_apology(&changed)).is_err()
            );
        }
    }
}

#[test]
fn apology_keeps_all_6216_published_english_first_seen_ranks() {
    let previous = apology_resource::without_apology(RESOURCES[0].0);
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
fn apology_english_six_branches_partial_full_and_literal_commit_once() {
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
fn apology_chinese_joined_separated_tones_uppercase_han_and_partials() {
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
fn apology_japanese_romaji_kana_case_full_and_partial_with_literal_raw() {
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
    // This shortened reading also names an older Word. Keep it selectable;
    // no authored continuation is invented for the older 再送信 entry.
    for seed in ["saiso", "SAISO", "さいそ"] {
        let ime = SUITE.engine("ja", seed);
        assert!(ime.candidates().iter().any(|candidate| {
            candidate.text == "再送信" && candidate.kind == CandidateKind::Word
        }));
        SUITE.commit("ja", seed, "再送信");
    }
    for seed in ["saisoushin", "saisoushinn", "saisoushin'", "さいそうしん"] {
        probe("ja", seed, "再送信", &[]);
    }
}

#[test]
fn apology_six_horizontal_gaps_keep_finite_authored_choices() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = ["sorry", "for", "the", "late", "reply,", "i", "was", "bus"].join(gap);
        let word = seed.strip_suffix("bus").unwrap().to_owned() + "busy";
        probe(
            "en",
            &seed,
            &word,
            &[&(word.clone() + " with work and missed your message.")],
        );
        for &(reading, _, word, authors) in ZH {
            probe("zh-Hans", &reading.replace('\'', gap), word, &authors);
        }
        for (i, (a, b)) in [("sha", "zai"), ("sai", "sou"), ("oshira", "se")]
            .into_iter()
            .enumerate()
        {
            probe("ja", &format!("{a}{gap}{b}"), JA[i].2, &JA[i].3);
        }
    }
}

#[test]
fn apology_adoption_and_long_english_draft_preserve_uncommitted_prefix() {
    for (language, entries) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(reading, _, word, authors) in entries {
            SUITE.adopt(language, reading, word, &authors);
        }
    }
    let prefix = "前面的文字🙂，保持原样。".repeat(35);
    let seed = format!("{prefix}sorry for the late reply, i was bus");
    let word = format!("{prefix}sorry for the late reply, i was busy");
    probe(
        "en",
        &seed,
        &word,
        &[&(word.clone() + " with work and missed your message.")],
    );
}

#[test]
fn apology_seven_line_breaks_and_opaque_tokens_keep_literal_raw_rescue() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, authors) in [
            (
                "en",
                format!("sorry for the late reply,{line}i was bus"),
                EN[0].3,
            ),
            ("zh-Hans", format!("lou{line}kan xiao xi"), ZH[0].3),
            ("ja", format!("sha{line}zai"), JA[0].3),
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
        ("en", "sorry for the late reply, i was field_bus"),
        (
            "en",
            "https://example.invalid/sorry_for_the_late_reply_i_was_bus",
        ),
        ("zh-Hans", "user_lou_kan_xiao_xi_42"),
        ("ja", "user_shazai_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
    for seed in [
        "sorry for the late reply, i was busz",
        "sorry for the late reply, i was awaz",
        "thanks for pointing that out, i'll checz",
        "thanks for pointing that out, i'll fixz",
        "that was my fault, i'll correcz",
        "that was my fault, i'll explaiz",
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
fn apology_synthetic_pending_disabled_errors_and_empty_models_keep_local_choices() {
    SUITE.missing_models(
        "en",
        "sorry for the late reply, i was bus",
        "sorry for the late reply, i was busy",
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
                    "when you get home, send me a messag",
                    "when you get home, send me a message",
                    "when you get home, send me a message so i know you're safe.",
                ),
                (
                    "did you make it hom",
                    "did you make it home",
                    "did you make it home safely after the long trip?",
                ),
                (
                    "just letting you know we're saf",
                    "just letting you know we're safe",
                    "just letting you know we're safe and everyone is fine.",
                ),
            ] {
                probe(language, seed, word, &[author]);
            }
        }
        "zh-Hans" => probe(
            language,
            "bie'dan'xin",
            "别担心",
            &[
                "别担心，我们可以一件一件慢慢处理。",
                "别担心，有需要的话我可以陪你一起去。",
            ],
        ),
        "ja" => probe(
            language,
            "ogenkidesuka",
            "お元気ですか",
            &[
                "お元気ですか。最近、無理をしていませんか。",
                "お元気ですか。返事は落ち着いたときで大丈夫です。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn apology_previous_neighbor_controls_still_commit_once() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn apology_old_and_new_choices_coexist_with_private_8_8_9_packs() {
    const NAME: &str = "apology_old_and_new_choices_coexist_with_private_8_8_9_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_APOLOGY_DIALOGUE_TEST_CHILD",
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

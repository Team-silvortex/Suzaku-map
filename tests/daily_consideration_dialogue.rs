//! Finite polite declines and consideration, not preference inference or task execution.
#[path = "support/consideration_resource.rs"]
mod consideration_resource;
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

const SUITE: Suite = Suite {
    marker: "CONSIDERATION",
};
// Authored expectations are fixed independently of resource parsing and candidate output.
const EN: &[EnglishCase] = &[
    (
        "thanks for the offer, but i'd rather",
        ["wait", "stay"],
        ["wai", "sta"],
        [
            "thanks for the offer, but i'd rather wait until later.",
            "thanks for the offer, but i'd rather stay home tonight.",
        ],
    ),
    (
        "you can take your time and",
        ["reply", "decide"],
        ["repl", "decid"],
        [
            "you can take your time and reply when you are free.",
            "you can take your time and decide when you are ready.",
        ],
    ),
    (
        "please don't feel bad about",
        ["saying", "asking"],
        ["sayin", "askin"],
        [
            "please don't feel bad about saying no if you are busy.",
            "please don't feel bad about asking for help when you need it.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "xin'yi'ling'le",
        "xin'yi'ling'l",
        "心意领了",
        ["心意领了，这次就不麻烦你了。", "心意领了，东西你留着用吧。"],
    ),
    (
        "bu'yong'te'yi",
        "bu'yong'te'y",
        "不用特意",
        [
            "不用特意跑一趟，等下次见面再说吧。",
            "不用特意准备东西，过来坐坐就好。",
        ],
    ),
    (
        "kan'ni'fang'bian",
        "kan'ni'fang'bia",
        "看你方便",
        [
            "看你方便，不着急，晚点回复也可以。",
            "看你方便，有空的时候再一起商量。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "yukkuri",
        "yukkur",
        "ゆっくり",
        [
            "ゆっくりで大丈夫です。急がなくても構いません。",
            "ゆっくり考えてから、返事をもらえれば大丈夫です。",
        ],
    ),
    (
        "yoroshikereba",
        "yoroshikereb",
        "よろしければ",
        [
            "よろしければ、都合のよいときに返事をください。",
            "よろしければ、無理のない範囲で手伝ってください。",
        ],
    ),
    (
        "goshinsetsu",
        "goshinsets",
        "ご親切",
        [
            "ご親切にありがとうございます。今回は自分でやってみます。",
            "ご親切なお申し出ですが、今日は大丈夫です。",
        ],
    ),
];
const TONES: [&str; 3] = [
    "xin1'yi4'ling3'le5",
    "bu4'yong4'te4'yi4",
    "kan4'ni3'fang1'bian4",
];
const KANA: [&str; 3] = ["ゆっくり", "よろしければ", "ごしんせつ"];
const PARTIAL_KANA: [&str; 3] = ["ゆっくr", "よろしけれb", "ごしんせts"];
const RESOURCES: &[(&str, &str)] = &[
    (
        include_str!("../data/lexicons/en.json"),
        "acc2f027d7350582293b781956277f3bb960a55446bf53a0fdbd7f74f27d0d8d",
    ),
    (
        include_str!("../data/lexicons/zh-Hans.json"),
        "4ce6ccbf721e39f37236f40f5ee57eddc6130b07d6415067415e627b42dec39b",
    ),
    (
        include_str!("../data/lexicons/ja.json"),
        "a476577e9d683001650c4993738f20f0578d0f82ae5ed6c5f8a4450495bd64e1",
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
fn consideration_appends_nine_contexts_eighteen_authors_with_exact_rescheduling_inverse() {
    for &(raw, expected_sha) in RESOURCES {
        let previous = consideration_resource::without_consideration(raw);
        SUITE.preserve_resource(
            &previous,
            "absent-consideration",
            "absent-end",
            expected_sha,
        );
        assert_eq!(
            consideration_resource::without_consideration(&previous),
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
        assert_eq!(layer.next_words[15 + i].0, context);
        assert_eq!(layer.next_words[15 + i].1, words);
        assert_eq!(&layer.sentences[30 + 2 * i..32 + 2 * i], authors);
    }
    for (language, entries, readings, start, contexts, authors) in [
        ("zh-Hans", ZH, 3442, 921, 937, 1872),
        ("ja", JA, 638, 409, 425, 849),
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
fn consideration_inverse_rejects_changed_old_bytes_mutations_removals_and_unknown_future() {
    for (&(raw, _), new_author) in RESOURCES.iter().zip([EN[0].3[0], ZH[0].3[0], JA[0].3[0]]) {
        let changed = raw.replacen("\"language\": ", "\"language\":  ", 1);
        assert_ne!(changed, raw);
        assert!(
            std::panic::catch_unwind(|| consideration_resource::without_consideration(&changed))
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
            std::panic::catch_unwind(|| consideration_resource::without_consideration(&changed))
                .is_err()
        );
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        for mutation in 0..3 {
            let mut changed = value.clone();
            if value["language"] == "en" {
                let layer = &mut changed["word_layers"][63];
                match mutation {
                    0 => {
                        layer["next_words"].as_array_mut().unwrap().remove(15);
                    }
                    1 => {
                        layer["sentences"].as_array_mut().unwrap().remove(30);
                    }
                    _ => layer["next_words"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(["future context", ["future"]])),
                }
            } else {
                let start = if value["language"] == "ja" { 409 } else { 921 };
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
                std::panic::catch_unwind(|| consideration_resource::without_consideration(
                    &changed
                ))
                .is_err()
            );
        }
    }
}

#[test]
fn consideration_keeps_all_6216_published_english_first_seen_ranks() {
    let previous = consideration_resource::without_consideration(RESOURCES[0].0);
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
fn consideration_english_six_branches_partial_full_and_literal_commit_once() {
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
fn consideration_chinese_joined_separated_tones_uppercase_han_and_partials() {
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
fn consideration_japanese_romaji_kana_case_full_and_partial_with_literal_raw() {
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
fn consideration_six_horizontal_gaps_keep_finite_authored_choices() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = [
            "thanks", "for", "the", "offer,", "but", "i'd", "rather", "wai",
        ]
        .join(gap);
        let word = seed.strip_suffix("wai").unwrap().to_owned() + "wait";
        probe("en", &seed, &word, &[&(word.clone() + " until later.")]);
        for &(reading, _, word, authors) in ZH {
            probe("zh-Hans", &reading.replace('\'', gap), word, &authors);
        }
        for (i, (a, b)) in [("yukku", "ri"), ("yoroshi", "kereba"), ("goshinse", "tsu")]
            .into_iter()
            .enumerate()
        {
            probe("ja", &format!("{a}{gap}{b}"), JA[i].2, &JA[i].3);
        }
    }
}

#[test]
fn consideration_adoption_and_long_english_draft_preserve_uncommitted_prefix() {
    for (language, entries) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(reading, _, word, authors) in entries {
            SUITE.adopt(language, reading, word, &authors);
        }
    }
    let prefix = "前面的文字🙂，保持原样。".repeat(35);
    let seed = format!("{prefix}thanks for the offer, but i'd rather wai");
    let word = format!("{prefix}thanks for the offer, but i'd rather wait");
    probe("en", &seed, &word, &[&(word.clone() + " until later.")]);
}

#[test]
fn consideration_seven_line_breaks_and_opaque_tokens_keep_literal_raw_rescue() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, authors) in [
            (
                "en",
                format!("thanks for the offer,{line}but i'd rather wai"),
                EN[0].3,
            ),
            ("zh-Hans", format!("xin{line}yi ling le"), ZH[0].3),
            ("ja", format!("yukku{line}ri"), JA[0].3),
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
        ("en", "thanks for the offer, but i'd rather field_wai"),
        ("en", "https://example.invalid/thanks_for_the_offer_wai"),
        ("zh-Hans", "user_xin_yi_ling_le_42"),
        ("ja", "user_yukkuri_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
    for seed in [
        "thanks for the offer, but i'd rather waiz",
        "thanks for the offer, but i'd rather staz",
        "you can take your time and replz",
        "you can take your time and decidz",
        "please don't feel bad about sayinz",
        "please don't feel bad about askinz",
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
fn consideration_synthetic_pending_disabled_errors_and_empty_models_keep_local_choices() {
    SUITE.missing_models(
        "en",
        "thanks for the offer, but i'd rather wai",
        "thanks for the offer, but i'd rather wait",
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
                    "something came up, could we meet tomorro",
                    "something came up, could we meet tomorrow",
                    "something came up, could we meet tomorrow at the same time?",
                ),
                (
                    "if you are running late, we can wai",
                    "if you are running late, we can wait",
                    "if you are running late, we can wait here for a few minutes.",
                ),
                (
                    "let's keep the same place and change the tim",
                    "let's keep the same place and change the time",
                    "let's keep the same place and change the time to three instead.",
                ),
            ] {
                probe(language, seed, word, &[author]);
            }
        }
        "zh-Hans" => probe(
            language,
            "lin'shi'you'an'pai",
            "临时有安排",
            &[
                "临时有安排，今天可能没法过去了。",
                "临时有安排，我们能换个时间吗？",
            ],
        ),
        "ja" => probe(
            language,
            "raishuu",
            "来週",
            &[
                "来週なら、今のところ火曜日が空いています。",
                "来週に変更しても大丈夫ですか。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn consideration_previous_neighbor_controls_still_commit_once() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn consideration_old_and_new_choices_coexist_with_private_8_8_9_packs() {
    const NAME: &str = "consideration_old_and_new_choices_coexist_with_private_8_8_9_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_CONSIDERATION_DIALOGUE_TEST_CHILD",
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

//! Finite work updates, scope questions, meeting coordination and handover feedback, not live actions.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/work_communication_resource.rs"]
mod work_communication_resource;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::BTreeMap;
use suzaku_map::{
    ime::candidate_mix::CandidateKind,
    languages::japanese::romaji_to_hiragana,
    lexicon::{Lexicon, WordLayer, builtin},
};

const SUITE: Suite = Suite {
    marker: "WORK_COMMUNICATION",
};
// Authored expectations are fixed independently of resource parsing and candidate output.
const EN: &[EnglishCase] = &[
    (
        "we're on track, but we still need",
        ["approval", "feedback"],
        ["approva", "feedbac"],
        [
            "we're on track, but we still need approval before we can continue.",
            "we're on track, but we still need feedback on the latest draft.",
        ],
    ),
    (
        "before we begin, can we agree on the",
        ["scope", "deadline"],
        ["scop", "deadlin"],
        [
            "before we begin, can we agree on the scope so everyone knows what to expect?",
            "before we begin, can we agree on the deadline and what needs to be ready?",
        ],
    ),
    (
        "for the next team meeting, should we",
        ["invite", "share"],
        ["invit", "shar"],
        [
            "for the next team meeting, should we invite everyone who needs to be there?",
            "for the next team meeting, should we share the draft before we talk?",
        ],
    ),
    (
        "once you have reviewed the handover, could you",
        ["confirm", "flag"],
        ["confir", "fla"],
        [
            "once you have reviewed the handover, could you confirm that everything is clear?",
            "once you have reviewed the handover, could you flag anything that is missing?",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "tong'bu'jin'du",
        "tong'bu'jin'd",
        "同步进度",
        [
            "同步进度：已经完成的部分我发给你，剩下的明天下午补上。",
            "同步进度时，也请说一下卡在哪里，我们一起想办法。",
        ],
    ),
    (
        "xu'qiu",
        "xu'qi",
        "需求",
        [
            "需求我先确认一下：这次只改文字，不调整页面，对吗？",
            "需求有变化的话，请把新增和删掉的部分分开标出来。",
        ],
    ),
    (
        "xian'shang'hui'yi",
        "xian'shang'hui'y",
        "线上会议",
        [
            "线上会议安排在明天下午三点，你看方便吗？",
            "线上会议开始前，麻烦先把要讨论的问题发到群里。",
        ],
    ),
    (
        "fan'kui'hui'zong",
        "fan'kui'hui'zon",
        "反馈汇总",
        [
            "反馈汇总已经整理好了，交接前请看一下还没解决的问题。",
            "反馈汇总里我标出了需要确认的地方，麻烦看完后告诉我。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "tantou",
        "tanto",
        "担当",
        [
            "担当している部分は、明日の午前中に仕上げる予定です。",
            "担当を引き継ぐ前に、今どこまで進んだかまとめます。",
        ],
    ),
    (
        "taishou",
        "taisho",
        "対象",
        [
            "対象になる作業と、今回は含めない作業を先に確認しましょう。",
            "対象はこの三つで合っていますか。必要な条件も教えてください。",
        ],
    ),
    (
        "kaigi",
        "kaig",
        "会議",
        [
            "会議は明日の午後三時からで、ご都合はいかがですか。",
            "会議の前に、話し合いたいことを二つだけ送ってもらえますか。",
        ],
    ),
    (
        "henkourireki",
        "henkourirek",
        "変更履歴",
        [
            "変更履歴を見ながら、直した箇所を一緒に確認しましょう。",
            "変更履歴に理由も残しておくので、引き継ぎのときに確認してください。",
        ],
    ),
];
const TONES: [&str; 4] = [
    "tong2'bu4'jin4'du4",
    "xu1'qiu2",
    "xian4'shang4'hui4'yi4",
    "fan3'kui4'hui4'zong3",
];
const KANA: [&str; 4] = ["たんとう", "たいしょう", "かいぎ", "へんこうりれき"];
const PARTIAL_KANA: [&str; 4] = ["たんと", "たいしょ", "かいg", "へんこうりれk"];
const PURE_KANA_NEAR: [&str; 4] = ["たんと", "たいしょ", "かい", "へんこうりれ"];
const RESOURCES: &[(&str, &str)] = &[
    (
        include_str!("../data/lexicons/en.json"),
        "9c6538551a3b80881b1f5db8a4c4d137ff0da33cbab1dfc89f0b682a974b2f63",
    ),
    (
        include_str!("../data/lexicons/zh-Hans.json"),
        "036f64fa73f0b3bca8b5a784c03263642c4bc02c371e62b3f452764fd3a1ad7b",
    ),
    (
        include_str!("../data/lexicons/ja.json"),
        "a254d6512fe44f81b1bed3c78c17a64583ad2a77d987205b8086cd90c05236f6",
    ),
];

fn probe(language: &str, seed: &str, word: &str, authors: &[&str]) {
    SUITE.choices(language, seed, word, authors);
    for text in [seed, word].into_iter().chain(authors.iter().copied()) {
        SUITE.commit(language, seed, text);
    }
}

// Coexistence is bounded, not a claim that an ambiguous old Word owns page one.
fn bounded_word(language: &str, seed: &str, word: &str) {
    let ime = SUITE.engine(language, seed);
    assert!(
        ime.candidates()
            .iter()
            .any(|candidate| { candidate.text == word && candidate.kind == CandidateKind::Word }),
        "missing bounded old Word {word:?} for {seed:?}: {:?}",
        ime.candidates()
    );
    SUITE.commit(language, seed, word);
}

fn chinese_ambiguous_requirements() {
    for seed in ["xu'qi", "XU QI", "xuqi"] {
        probe("zh-Hans", seed, "需求", &ZH[1].3);
        bounded_word("zh-Hans", seed, "续期");
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
fn work_communication_appends_twelve_contexts_twenty_four_authors_with_exact_apology_inverse() {
    for &(raw, expected_sha) in RESOURCES {
        let previous = work_communication_resource::without_work_communication(raw);
        SUITE.preserve_resource(
            &previous,
            "absent-work_communication",
            "absent-end",
            expected_sha,
        );
        assert_eq!(
            work_communication_resource::without_work_communication(&previous),
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
        assert_eq!(layer.next_words[27 + i].0, context);
        assert_eq!(layer.next_words[27 + i].1, words);
        assert_eq!(&layer.sentences[54 + 2 * i..56 + 2 * i], authors);
    }
    for (language, entries, readings, start, contexts, authors) in [
        ("zh-Hans", ZH, 3442, 933, 937, 1872),
        ("ja", JA, 638, 421, 425, 849),
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
            data.continuations().skip(start).take(4).zip(entries)
        {
            assert_eq!(word, expected);
            assert_eq!(pair, authors);
        }
    }
}

#[test]
fn work_communication_inverse_rejects_changed_old_bytes_mutations_removals_and_unknown_future() {
    for (&(raw, _), new_author) in RESOURCES.iter().zip([EN[0].3[0], ZH[0].3[0], JA[0].3[0]]) {
        let changed = raw.replacen("\"language\": ", "\"language\":  ", 1);
        assert_ne!(changed, raw);
        assert!(
            std::panic::catch_unwind(|| work_communication_resource::without_work_communication(
                &changed
            ))
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
            std::panic::catch_unwind(|| work_communication_resource::without_work_communication(
                &changed
            ))
            .is_err()
        );
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        for mutation in 0..3 {
            let mut changed = value.clone();
            if value["language"] == "en" {
                let layer = &mut changed["word_layers"][63];
                match mutation {
                    0 => {
                        layer["next_words"].as_array_mut().unwrap().remove(27);
                    }
                    1 => {
                        layer["sentences"].as_array_mut().unwrap().remove(54);
                    }
                    _ => layer["next_words"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(["future context", ["future"]])),
                }
            } else {
                let start = if value["language"] == "ja" { 421 } else { 933 };
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
                std::panic::catch_unwind(|| {
                    work_communication_resource::without_work_communication(&changed)
                })
                .is_err()
            );
        }
    }
}

#[test]
fn work_communication_keeps_all_6216_published_english_first_seen_ranks() {
    let previous = work_communication_resource::without_work_communication(RESOURCES[0].0);
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
fn work_communication_english_eight_branches_partial_full_and_literal_commit_once() {
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
fn work_communication_chinese_joined_separated_tones_uppercase_han_and_partials() {
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
    chinese_ambiguous_requirements();
}

#[test]
fn work_communication_japanese_romaji_kana_case_full_and_partial_with_literal_raw() {
    for (i, &(reading, partial, word, authors)) in JA.iter().enumerate() {
        assert_eq!(romaji_to_hiragana(reading), KANA[i]);
        assert_eq!(romaji_to_hiragana(partial), PARTIAL_KANA[i]);
        // かい is broad. Its positive pure-kana spelling is the full かいぎ;
        // keep the broad spelling for the separate literal/old-choice probe.
        let pure_positive = if i == 2 { KANA[i] } else { PURE_KANA_NEAR[i] };
        for seed in [
            reading.into(),
            reading.to_ascii_uppercase(),
            KANA[i].into(),
            partial.into(),
            partial.to_ascii_uppercase(),
            PARTIAL_KANA[i].into(),
            pure_positive.into(),
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
    for seed in ["kaig", "KAIg", "かいg"] {
        probe("ja", seed, "会議", &JA[2].3);
    }
    let broad = PURE_KANA_NEAR[2];
    assert_eq!(broad, "かい");
    let ime = SUITE.engine("ja", broad);
    assert!(
        ime.candidates().iter().any(|candidate| {
            candidate.text == broad && candidate.kind == CandidateKind::Literal
        })
    );
    SUITE.commit("ja", broad, broad);
    bounded_word("ja", broad, "買い物");
}

#[test]
fn work_communication_six_horizontal_gaps_keep_finite_authored_choices() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = [
            "we're", "on", "track,", "but", "we", "still", "need", "approva",
        ]
        .join(gap);
        let word = seed.strip_suffix("approva").unwrap().to_owned() + "approval";
        probe(
            "en",
            &seed,
            &word,
            &[&(word.clone() + " before we can continue.")],
        );
        for &(reading, _, word, authors) in ZH {
            probe("zh-Hans", &reading.replace('\'', gap), word, &authors);
        }
        for (i, (a, b)) in [
            ("tan", "tou"),
            ("tai", "shou"),
            ("kai", "gi"),
            ("henkou", "rireki"),
        ]
        .into_iter()
        .enumerate()
        {
            probe("ja", &format!("{a}{gap}{b}"), JA[i].2, &JA[i].3);
        }
    }
}

#[test]
fn work_communication_adoption_and_long_english_draft_preserve_uncommitted_prefix() {
    for (language, entries) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(reading, _, word, authors) in entries {
            SUITE.adopt(language, reading, word, &authors);
        }
    }
    let prefix = "前面的文字🙂，保持原样。".repeat(35);
    let seed = format!("{prefix}we're on track, but we still need approva");
    let word = format!("{prefix}we're on track, but we still need approval");
    probe(
        "en",
        &seed,
        &word,
        &[&(word.clone() + " before we can continue.")],
    );
}

#[test]
fn work_communication_seven_line_breaks_and_opaque_tokens_keep_literal_raw_rescue() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, authors) in [
            (
                "en",
                format!("we're on track,{line}but we still need approva"),
                EN[0].3,
            ),
            ("zh-Hans", format!("tong{line}bu jin du"), ZH[0].3),
            ("ja", format!("tan{line}tou"), JA[0].3),
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
        ("en", "we're on track, but we still need field_approva"),
        (
            "en",
            "https://example.invalid/we_are_on_track_but_we_still_need_approva",
        ),
        ("zh-Hans", "user_tong_bu_jin_du_42"),
        ("ja", "user_tantou_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
    for seed in [
        "we're on track, but we still need approvaz",
        "we're on track, but we still need feedbacz",
        "before we begin, can we agree on the scopz",
        "before we begin, can we agree on the deadlinz",
        "for the next team meeting, should we invitz",
        "for the next team meeting, should we sharz",
        "once you have reviewed the handover, could you confirz",
        "once you have reviewed the handover, could you flaz",
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
fn work_communication_synthetic_pending_disabled_errors_and_empty_models_keep_local_choices() {
    SUITE.missing_models(
        "en",
        "we're on track, but we still need approva",
        "we're on track, but we still need approval",
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
                (
                    "the first part is don",
                    "the first part is done",
                    "the first part is done, and i'm checking the rest.",
                ),
                (
                    "before we start, i want to confirm wha",
                    "before we start, i want to confirm what",
                    "before we start, i want to confirm what you need.",
                ),
                (
                    "for our next meeting, would you prefer mornin",
                    "for our next meeting, would you prefer morning",
                    "for our next meeting, would you prefer morning or afternoon?",
                ),
                (
                    "please review the handover note",
                    "please review the handover notes",
                    "please review the handover notes.",
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
fn work_communication_previous_neighbor_controls_still_commit_once() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn work_communication_old_and_new_choices_coexist_with_private_8_8_9_packs() {
    const NAME: &str = "work_communication_old_and_new_choices_coexist_with_private_8_8_9_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_WORK_COMMUNICATION_DIALOGUE_TEST_CHILD",
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
                if language == "zh-Hans" {
                    chinese_ambiguous_requirements();
                } else {
                    // Only the private Japanese snapshot includes the old
                    // work-pack Word 担当者. It has no authored pair to invent.
                    for seed in ["tantou", "TANTOU", "たんとう", "tanto", "TANTO", "たんと"]
                    {
                        bounded_word("ja", seed, "担当者");
                    }
                    for seed in ["tantousha", "たんとうしゃ"] {
                        bounded_word("ja", seed, "担当者");
                    }
                }
            }
        },
    );
}

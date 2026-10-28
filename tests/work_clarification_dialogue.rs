//! Finite work clarification and asynchronous reply boundaries, not message delivery or work execution.
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/work_clarification_resource.rs"]
mod work_clarification_resource;
use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use ring::digest::{SHA256, digest};
use std::collections::BTreeMap;
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, PAGE_SIZE},
    languages::japanese::romaji_to_hiragana,
    lexicon::{Lexicon, WordLayer, builtin},
};

const SUITE: Suite = Suite {
    marker: "WORK_CLARIFICATION",
};
// Authored expectations are fixed independently of resource parsing and candidate output.
const EN: &[EnglishCase] = &[
    (
        "before i make the change, could you confirm",
        ["which", "what"],
        ["whic", "wha"],
        [
            "before i make the change, could you confirm which version we should use?",
            "before i make the change, could you confirm what needs to stay the same?",
        ],
    ),
    (
        "about your feedback, could you",
        ["show", "explain"],
        ["sho", "explai"],
        [
            "about your feedback, could you show me which part needs work?",
            "about your feedback, could you explain why you would prefer a different option?",
        ],
    ),
    (
        "reply when you've had time to",
        ["check", "finish"],
        ["chec", "finis"],
        [
            "reply when you've had time to check the details with the team.",
            "reply when you've had time to finish what you're working on.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "dai'que'ren",
        "dai'que're",
        "待确认",
        [
            "待确认的地方我已经标出来了，麻烦你方便时看一下。",
            "待确认的问题先保留，核对清楚后我们再继续。",
        ],
    ),
    (
        "xiu'gai'yi'jian",
        "xiu'gai'yi'jia",
        "修改意见",
        [
            "修改意见可以具体到段落吗？这样我比较容易定位。",
            "修改意见里请说明哪里要改，哪些内容需要保持原样。",
        ],
    ),
    (
        "bu'ji'zhe'hui'fu",
        "bu'ji'zhe'hui'f",
        "不急着回复",
        [
            "不急着回复，先把手头的事情忙完就好。",
            "不急着回复，等你和同事核对清楚后再告诉我。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "yakuwari",
        "yakuwar",
        "役割",
        [
            "役割を分ける前に、誰が何を担当するか確認しましょう。",
            "役割が重なっているところは、一度整理してから進めませんか。",
        ],
    ),
    (
        "yougo",
        "youg",
        "用語",
        [
            "用語の意味が少し曖昧なので、具体例を一つ教えてください。",
            "用語の使い方をそろえるために、この資料の説明を確認しましょう。",
        ],
    ),
    (
        "jouhou",
        "jouho",
        "情報",
        [
            "情報を確認してからで大丈夫です。分かったところだけ返事をください。",
            "情報がまだ足りなければ、返事は明日でも構いません。",
        ],
    ),
];
const TONES: [&str; 3] = [
    "dai4'que4'ren4",
    "xiu1'gai3'yi4'jian4",
    "bu4'ji2'zhe5'hui2'fu4",
];
const KANA: [&str; 3] = ["やくわり", "ようご", "じょうほう"];
const PARTIAL_KANA: [&str; 3] = ["やくわr", "ようg", "じょうほ"];
// The positive pure-kana spelling for 用語 is complete ようご, not broad よう.
const PURE_KANA_NEAR: [&str; 3] = ["やくわ", "ようご", "じょうほ"];
const ROMAJI_GAP_PARTS: [(&str, &str); 3] = [("yaku", "wari"), ("you", "go"), ("jou", "hou")];
const WRONG_TAILS: [[&str; 2]; 3] = [["whicx", "whax"], ["shox", "explaix"], ["checx", "finisx"]];
const RESOURCES: &[(&str, &str)] = &[
    (
        include_str!("../data/lexicons/en.json"),
        "8cafe0c4c7f47de2db83ec588a6399442e3fbab0bbc5cde43d088662a218c0fb",
    ),
    (
        include_str!("../data/lexicons/zh-Hans.json"),
        "1f569d38223393870c8c6b6f41a817f4c068126eae2b001d43533f97ee0ee288",
    ),
    (
        include_str!("../data/lexicons/ja.json"),
        "cb99141e60f2ada2f5f1fc2ff557abec456053905ea4289ad6ce49cd0c3992d3",
    ),
];

fn probe(language: &str, seed: &str, word: &str, authors: &[&str]) {
    if language == "zh-Hans" && seed == "xiu'gai'yi'jia" {
        // jia fully spells the older 衣架 as well as prefixing 意见 (jian).
        // Keep that real ambiguity and the six-row page boundary, rather than
        // promising every continuation on page one for an unfinished syllable.
        let ime = SUITE.engine(language, seed);
        assert_eq!(PAGE_SIZE, 6);
        assert_eq!(ime.candidates().len(), 10);
        assert_eq!(word, "修改意见");
        assert_eq!(authors, ZH[1].3);
        let old = &ime.candidates()[0];
        assert_eq!(old.text, "修改衣架");
        assert_eq!(old.kind, CandidateKind::Sentence);
        assert_eq!(old.score.to_bits(), 100.0_f32.to_bits());
        assert_eq!(SUITE.first_page(&ime, word, CandidateKind::Word), 1);
        for (offset, author) in authors.iter().enumerate() {
            let candidate = &ime.candidates()[5 + offset];
            assert_eq!(candidate.text, *author);
            assert_eq!(candidate.kind, CandidateKind::Sentence);
            assert_eq!(
                candidate.score.to_bits(),
                (78.5 - offset as f32 * 0.5).to_bits()
            );
        }
        SUITE.commit(language, seed, "修改衣架");
    } else {
        SUITE.choices(language, seed, word, authors);
    }
    for text in [seed, word].into_iter().chain(authors.iter().copied()) {
        SUITE.commit(language, seed, text);
    }
}

// Broad よう also names older entries; do not demand the new authors here.
fn japanese_broad_boundary() {
    let seed = "よう";
    let ime = SUITE.engine("ja", seed);
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == seed && c.kind == CandidateKind::Literal)
    );
    SUITE.commit("ja", seed, seed);
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == "要点" && c.kind == CandidateKind::Word)
    );
    SUITE.commit("ja", seed, "要点");
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
fn work_clarification_appends_nine_contexts_eighteen_authors_with_exact_work_communication_inverse()
{
    for &(raw, expected_sha) in RESOURCES {
        let previous = work_clarification_resource::without_work_clarification(raw);
        SUITE.preserve_resource(
            &previous,
            "absent-work_clarification",
            "absent-end",
            expected_sha,
        );
        assert_eq!(
            work_clarification_resource::without_work_clarification(&previous),
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
        (64, 968, 1900)
    );
    let layer = &en.word_layers()[63];
    assert_eq!(layer.id, "daily_presence_dialogue");
    assert!(layer.words.is_empty());
    assert_eq!((layer.next_words.len(), layer.sentences.len()), (53, 106));
    for (i, &(context, words, _, authors)) in EN.iter().enumerate() {
        assert_eq!(layer.next_words[31 + i].0, context);
        assert_eq!(layer.next_words[31 + i].1, words);
        assert_eq!(&layer.sentences[62 + 2 * i..64 + 2 * i], authors);
    }
    for (language, entries, readings, start, contexts, authors) in [
        ("zh-Hans", ZH, 3442, 937, 959, 1916),
        ("ja", JA, 638, 425, 447, 893),
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
fn work_clarification_inverse_rejects_changed_old_bytes_mutations_removals_and_unknown_future() {
    for (&(raw, _), new_author) in RESOURCES.iter().zip([EN[0].3[0], ZH[0].3[0], JA[0].3[0]]) {
        let changed = raw.replacen("\"language\": ", "\"language\":  ", 1);
        assert_ne!(changed, raw);
        assert!(
            std::panic::catch_unwind(|| work_clarification_resource::without_work_clarification(
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
            std::panic::catch_unwind(|| work_clarification_resource::without_work_clarification(
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
                        layer["next_words"].as_array_mut().unwrap().remove(31);
                    }
                    1 => {
                        layer["sentences"].as_array_mut().unwrap().remove(62);
                    }
                    _ => layer["next_words"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(["future context", ["future"]])),
                }
            } else {
                let start = if value["language"] == "ja" { 425 } else { 937 };
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
                    work_clarification_resource::without_work_clarification(&changed)
                })
                .is_err()
            );
        }
    }
}

#[test]
fn work_clarification_keeps_all_6216_published_english_first_seen_ranks() {
    let previous = work_clarification_resource::without_work_clarification(RESOURCES[0].0);
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
fn work_clarification_english_six_branches_partial_full_and_literal_commit_once() {
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
fn work_clarification_chinese_joined_separated_tones_uppercase_han_and_partials() {
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
fn work_clarification_japanese_romaji_kana_case_full_and_partial_with_literal_raw() {
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
    japanese_broad_boundary();
}

#[test]
fn work_clarification_six_horizontal_gaps_keep_finite_authored_choices() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        let seed = [
            "before", "i", "make", "the", "change,", "could", "you", "confirm", "whic",
        ]
        .join(gap);
        let word = seed.strip_suffix("whic").unwrap().to_owned() + "which";
        probe(
            "en",
            &seed,
            &word,
            &[&(word.clone() + " version we should use?")],
        );
        for &(reading, _, word, authors) in ZH {
            probe("zh-Hans", &reading.replace('\'', gap), word, &authors);
        }
        for (i, (a, b)) in ROMAJI_GAP_PARTS.into_iter().enumerate() {
            probe("ja", &format!("{a}{gap}{b}"), JA[i].2, &JA[i].3);
        }
    }
}

#[test]
fn work_clarification_adoption_and_long_english_draft_preserve_uncommitted_prefix() {
    for (language, entries) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(reading, _, word, authors) in entries {
            SUITE.adopt(language, reading, word, &authors);
        }
    }
    let prefix = "前面的文字🙂，保持原样。".repeat(35);
    let seed = format!("{prefix}before i make the change, could you confirm whic");
    let word = format!("{prefix}before i make the change, could you confirm which");
    probe(
        "en",
        &seed,
        &word,
        &[&(word.clone() + " version we should use?")],
    );
}

#[test]
fn work_clarification_seven_line_breaks_and_opaque_tokens_keep_literal_raw_rescue() {
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (language, seed, authors) in [
            (
                "en",
                format!("before i make the change,{line}could you confirm whic"),
                EN[0].3,
            ),
            ("zh-Hans", format!("dai{line}que ren"), ZH[0].3),
            ("ja", format!("yaku{line}wari"), JA[0].3),
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
        (
            "en",
            "before i make the change, could you confirm field_whic",
        ),
        (
            "en",
            "https://example.invalid/before_i_make_the_change_could_you_confirm_whic",
        ),
        ("zh-Hans", "user_dai_que_ren_42"),
        ("ja", "user_yakuwari_42"),
    ] {
        SUITE.commit(language, seed, seed);
    }
    for (i, tails) in WRONG_TAILS.iter().enumerate() {
        for tail in tails {
            let seed = format!("{} {tail}", EN[i].0);
            assert!(!SUITE.engine("en", &seed).candidates().iter().any(|c| {
                EN.iter().any(|&(context, words, _, authors)| {
                    authors.contains(&c.text.as_str())
                        || words
                            .iter()
                            .any(|word| c.text == format!("{context} {word}"))
                })
            }));
            SUITE.commit("en", &seed, &seed);
        }
    }
}

#[test]
fn work_clarification_synthetic_pending_disabled_errors_and_empty_models_keep_local_choices() {
    SUITE.missing_models(
        "en",
        "before i make the change, could you confirm whic",
        "before i make the change, could you confirm which",
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
                    "before we start, i want to confirm wha",
                    "before we start, i want to confirm what",
                    "before we start, i want to confirm what you need.",
                ),
                (
                    "could you clarify the prioritie",
                    "could you clarify the priorities",
                    "could you clarify the priorities?",
                ),
                (
                    "you can take your time and repl",
                    "you can take your time and reply",
                    "you can take your time and reply when you are free.",
                ),
            ] {
                probe(language, seed, word, &[author]);
            }
        }
        "zh-Hans" => probe(
            language,
            "xu'qiu",
            "需求",
            &[
                "需求我先确认一下：这次只改文字，不调整页面，对吗？",
                "需求有变化的话，请把新增和删掉的部分分开标出来。",
            ],
        ),
        "ja" => probe(
            language,
            "henkourireki",
            "変更履歴",
            &[
                "変更履歴を見ながら、直した箇所を一緒に確認しましょう。",
                "変更履歴に理由も残しておくので、引き継ぎのときに確認してください。",
            ],
        ),
        _ => unreachable!(),
    }
}

#[test]
fn work_clarification_previous_neighbor_controls_still_commit_once() {
    for language in ["en", "zh-Hans", "ja"] {
        old_controls(language);
    }
}

#[test]
fn work_clarification_old_and_new_choices_coexist_with_private_8_8_9_packs() {
    const NAME: &str = "work_clarification_old_and_new_choices_coexist_with_private_8_8_9_packs";
    SUITE.private_packs(
        NAME,
        "SUZAKU_WORK_CLARIFICATION_DIALOGUE_TEST_CHILD",
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
            if language == "ja" {
                japanese_broad_boundary();
            }
        },
    );
}

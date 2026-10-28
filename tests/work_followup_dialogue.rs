//! Work follow-up candidate and commit coverage with an unavailable model.
// This suite uses a subset of the shared integration helpers.
#[allow(dead_code)]
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/work_followup_resource.rs"]
mod work_followup_resource;

use daily_dialogue::{EnglishCase, ReadingCase, Suite};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{PredictionStatus, candidate_mix::CandidateKind},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
};

const SUITE: Suite = Suite {
    marker: "WORK_FOLLOWUP",
};
const EN: &[EnglishCase] = &[
    (
        "after the meeting, i'll",
        ["send", "update"],
        ["sen", "updat"],
        [
            "after the meeting, i'll send the action items to everyone.",
            "after the meeting, i'll update the action items with the owners.",
        ],
    ),
    (
        "if the deadline changes, please",
        ["tell", "let"],
        ["tel", "le"],
        [
            "if the deadline changes, please tell me as soon as you know.",
            "if the deadline changes, please let the team know before the end of the day.",
        ],
    ),
    (
        "here is a quick update on",
        ["what", "where"],
        ["wha", "wher"],
        [
            "here is a quick update on what is done and what still needs work.",
            "here is a quick update on where we are and what comes next.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "qi'xian",
        "qi'xia",
        "期限",
        [
            "期限可能赶不上，我今天会先说明原因和预计完成时间。",
            "期限可以调整到下周吗？我会先整理好已完成的部分。",
        ],
    ),
    (
        "jie'zhi'ri'qi",
        "jie'zhi'ri'q",
        "截止日期",
        [
            "截止日期是周五中午，我会提前把材料发给你。",
            "截止日期如果有变化，请及时告诉我，我好调整安排。",
        ],
    ),
    (
        "zui'xin'jin'zhan",
        "zui'xin'jin'zha",
        "最新进展",
        [
            "最新进展是主要内容已经完成，剩下的问题正在核对。",
            "最新进展我会在今天下班前发到群里。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "shinchoku",
        "shincho",
        "進捗",
        [
            "進捗を共有します。作業は予定どおり進んでいます。",
            "進捗に変化があれば、分かった時点でお知らせします。",
        ],
    ),
    (
        "hikitsugi",
        "hikitsug",
        "引き継ぎ",
        [
            "引き継ぎの前に、未完了の作業と担当者をまとめます。",
            "引き継ぎに必要な資料は、明日までに共有します。",
        ],
    ),
    (
        "soudan",
        "souda",
        "相談",
        [
            "相談したいことがあります。納期について少しお時間をいただけますか。",
            "相談の結果、作業の優先順位を見直すことになりました。",
        ],
    ),
];

#[test]
fn appended_data_reconstructs_the_entire_previous_resources() {
    for (raw, new_author) in [
        (include_str!("../data/lexicons/en.json"), EN[0].3[0]),
        (include_str!("../data/lexicons/zh-Hans.json"), ZH[0].3[0]),
        (include_str!("../data/lexicons/ja.json"), JA[0].3[0]),
    ] {
        let previous = work_followup_resource::without_work_followup(raw);
        assert!(!previous.contains(new_author));
        assert_eq!(
            work_followup_resource::without_work_followup(&previous),
            previous
        );
        let mutated = raw.replacen(new_author, "unexpected changed sentence", 1);
        assert_ne!(mutated, raw);
        assert!(
            std::panic::catch_unwind(|| {
                work_followup_resource::without_work_followup(&mutated)
            })
            .is_err()
        );
    }
}

fn contains(language: &str, seed: &str, text: &str, kind: CandidateKind) {
    let ime = SUITE.engine(language, seed);
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == text && c.kind == kind),
        "missing {kind:?} {text:?} for {language} {seed:?}: {:?}",
        ime.candidates()
    );
}

#[test]
fn english_branches_include_local_words_sentences_and_literal_and_commit_once() {
    for &(context, words, partials, authors) in EN {
        for i in 0..2 {
            let full = format!("{context} {}", words[i]);
            let partial = format!("{context} {}", partials[i]);
            for seed in [&full, &partial] {
                contains("en", seed, &full, CandidateKind::Word);
                SUITE.commit("en", seed, seed);
            }
            contains("en", &full, authors[i], CandidateKind::Sentence);
            SUITE.commit("en", &full, &full);
            SUITE.commit("en", &full, authors[i]);
        }
    }
}

#[test]
fn chinese_and_japanese_complete_and_near_complete_inputs_remain_selectable() {
    for (language, cases) in [("zh-Hans", ZH), ("ja", JA)] {
        for &(full, partial, word, authors) in cases {
            for seed in [full, partial] {
                contains(language, seed, word, CandidateKind::Word);
                SUITE.commit(language, seed, seed);
                SUITE.commit(language, seed, word);
            }
            for author in authors {
                contains(language, full, author, CandidateKind::Sentence);
                SUITE.commit(language, full, author);
            }
        }
    }
}

#[test]
fn chinese_tones_spaces_and_japanese_kana_keep_the_new_candidates() {
    let tones = ["qi1'xian4", "jie2'zhi3'ri4'qi1", "zui4'xin1'jin4'zhan3"];
    for (i, &(reading, _, word, authors)) in ZH.iter().enumerate() {
        for seed in [
            reading.replace('\'', " ").to_ascii_uppercase(),
            tones[i].to_owned(),
        ] {
            contains("zh-Hans", &seed, word, CandidateKind::Word);
            for author in authors {
                contains("zh-Hans", &seed, author, CandidateKind::Sentence);
            }
        }
    }
    for (i, seed) in ["しんちょく", "ひきつぎ", "そうだん"]
        .into_iter()
        .enumerate()
    {
        contains("ja", seed, JA[i].2, CandidateKind::Word);
        for author in JA[i].3 {
            contains("ja", seed, author, CandidateKind::Sentence);
        }
    }
}

#[test]
fn recommended_offline_packs_preserve_old_and_new_choices() {
    const NAME: &str = "recommended_offline_packs_preserve_old_and_new_choices";
    SUITE.private_packs(
        NAME,
        "SUZAKU_WORK_FOLLOWUP_TEST_CHILD",
        |language| match language {
            "en" => contains("en", "reply", "reply", CandidateKind::Word),
            "zh-Hans" => contains("zh-Hans", "xu'qiu", "需求", CandidateKind::Word),
            "ja" => contains("ja", "henkourireki", "変更履歴", CandidateKind::Word),
            _ => unreachable!(),
        },
        |language| match language {
            "en" => {
                for &(context, words, _, authors) in EN {
                    for i in 0..2 {
                        let seed = format!("{context} {}", words[i]);
                        contains(language, &seed, authors[i], CandidateKind::Sentence);
                    }
                }
            }
            "zh-Hans" | "ja" => {
                for &(reading, _, word, authors) in if language == "ja" { JA } else { ZH } {
                    contains(language, reading, word, CandidateKind::Word);
                    for author in authors {
                        contains(language, reading, author, CandidateKind::Sentence);
                    }
                }
            }
            _ => unreachable!(),
        },
    );
}

struct Unavailable;
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "work-followup-unavailable"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        Err(LlmProviderError::Unavailable)
    }
}

#[test]
fn unavailable_model_preserves_local_candidates_in_each_language() {
    for (language, seed) in [
        ("en", "after the meeting, i'll send"),
        ("zh-Hans", "jie'zhi'ri'qi"),
        ("ja", "shinchoku"),
    ] {
        let baseline = SUITE.engine(language, seed).candidates().to_vec();
        let mut ime = SUITE.engine(language, "");
        ime.configure_prediction(Some(Arc::new(Unavailable)));
        ime.seed(seed);
        assert_eq!(ime.candidates(), baseline);
        let deadline = Instant::now() + Duration::from_secs(2);
        while ime.prediction_pending() && Instant::now() < deadline {
            ime.poll_prediction();
            assert_eq!(ime.candidates(), baseline);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!ime.prediction_pending());
        assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
        assert_eq!(ime.prediction_error(), Some(&LlmProviderError::Unavailable));
        assert_eq!(ime.candidates(), baseline);
    }
}

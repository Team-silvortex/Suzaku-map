//! Local work-access candidates and commits, including model-unavailable fallback.
#[allow(dead_code)]
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/work_access_resource.rs"]
mod work_access_resource;

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
    marker: "WORK_ACCESS",
};
const EN: &[EnglishCase] = &[
    (
        "i'm blocked until we receive",
        ["approval", "feedback"],
        ["approv", "feedbac"],
        [
            "i'm blocked until we receive approval to move forward.",
            "i'm blocked until we receive feedback on the latest draft.",
        ],
    ),
    (
        "when you have a moment, could you",
        ["review", "confirm"],
        ["revie", "confir"],
        [
            "when you have a moment, could you review the updated document?",
            "when you have a moment, could you confirm that the details are correct?",
        ],
    ),
    (
        "if you need more time, we can",
        ["adjust", "discuss"],
        ["adjus", "discus"],
        [
            "if you need more time, we can adjust the schedule together.",
            "if you need more time, we can discuss what to prioritize.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "xia'zai'lian'jie",
        "xia'zai'lian'ji",
        "下载链接",
        [
            "下载链接打不开，可以麻烦你再发一次吗？",
            "下载链接我已经收到了，等下载完成后再确认内容。",
        ],
    ),
    (
        "quan'xian'bu'zu",
        "quan'xian'bu'z",
        "权限不足",
        [
            "权限不足，我暂时打不开这个文件，能帮我检查一下吗？",
            "权限不足的话，我可以先查看你发来的预览。",
        ],
    ),
    (
        "wang'luo'duan'kai",
        "wang'luo'duan'ka",
        "网络断开",
        [
            "网络断开了，我重新连接后再把文件发给你。",
            "网络断开的时候我没听清，能麻烦你重复一下吗？",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "tenpufairu",
        "tenpufair",
        "添付ファイル",
        [
            "添付ファイルを受け取りました。内容を確認してからお返事します。",
            "添付ファイルが見当たらないので、もう一度送っていただけますか。",
        ],
    ),
    (
        "saisoushin",
        "saisoushi",
        "再送信",
        [
            "再送信しました。届いているか確認していただけますか。",
            "再送信する前に、宛先と添付ファイルを確認します。",
        ],
    ),
    (
        "tsuushin",
        "tsuushi",
        "通信",
        [
            "通信が不安定なので、接続を確認してからもう一度連絡します。",
            "通信が途切れて最後の部分が聞こえませんでした。もう一度お願いします。",
        ],
    ),
];

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
fn exact_inverse_restores_prior_resources_and_rejects_mutations() {
    for (raw, author) in [
        (include_str!("../data/lexicons/en.json"), EN[0].3[0]),
        (include_str!("../data/lexicons/zh-Hans.json"), ZH[0].3[0]),
        (include_str!("../data/lexicons/ja.json"), JA[0].3[0]),
    ] {
        let previous = work_access_resource::without_work_access(raw);
        assert!(!previous.contains(author));
        assert_eq!(
            work_access_resource::without_work_access(&previous),
            previous
        );
        let mutated = raw.replacen(author, "unexpected changed sentence", 1);
        assert_ne!(mutated, raw);
        assert!(
            std::panic::catch_unwind(|| work_access_resource::without_work_access(&mutated))
                .is_err()
        );
    }
}

#[test]
fn english_word_and_sentence_branches_are_selectable_and_commit_once() {
    for &(context, words, partials, authors) in EN {
        for i in 0..2 {
            let full = format!("{context} {}", words[i]);
            let partial = format!("{context} {}", partials[i]);
            for seed in [&full, &partial] {
                contains("en", seed, &full, CandidateKind::Word);
                SUITE.commit("en", seed, seed);
            }
            contains("en", &full, authors[i], CandidateKind::Sentence);
            SUITE.commit("en", &full, authors[i]);
        }
    }
}

#[test]
fn chinese_and_japanese_words_and_sentences_are_selectable_and_commit_once() {
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
fn japanese_kana_and_chinese_tones_retain_word_and_sentence_candidates() {
    for (i, seed) in [
        "xia4'zai4'lian4'jie1",
        "quan2'xian4'bu4'zu2",
        "wang3'luo4'duan4'kai1",
    ]
    .into_iter()
    .enumerate()
    {
        contains("zh-Hans", seed, ZH[i].2, CandidateKind::Word);
        for author in ZH[i].3 {
            contains("zh-Hans", seed, author, CandidateKind::Sentence);
        }
    }
    for (i, seed) in ["てんぷふぁいる", "さいそうしん", "つうしん"]
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
fn recommended_offline_packs_preserve_previous_and_new_choices() {
    const NAME: &str = "recommended_offline_packs_preserve_previous_and_new_choices";
    SUITE.private_packs(
        NAME,
        "SUZAKU_WORK_ACCESS_TEST_CHILD",
        |language| match language {
            "en" => contains("en", "reply", "reply", CandidateKind::Word),
            "zh-Hans" => contains("zh-Hans", "jie'zhi'ri'qi", "截止日期", CandidateKind::Word),
            "ja" => contains("ja", "shinchoku", "進捗", CandidateKind::Word),
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
        "work-access-unavailable"
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
fn unavailable_model_keeps_local_candidates_in_each_language() {
    for (language, seed, author) in [
        ("en", "i'm blocked until we receive approval", EN[0].3[0]),
        ("zh-Hans", "xia'zai'lian'jie", ZH[0].3[0]),
        ("ja", "tenpufairu", JA[0].3[0]),
    ] {
        contains(language, seed, author, CandidateKind::Sentence);
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

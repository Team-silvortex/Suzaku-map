//! Daily support candidates remain usable when predictions are unavailable.
#[allow(dead_code)]
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/daily_support_resource.rs"]
mod daily_support_resource;

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
    marker: "DAILY_SUPPORT",
};
const EN: &[EnglishCase] = &[
    (
        "before we order, could you check whether the",
        ["soup", "sauce"],
        ["sou", "sauc"],
        [
            "before we order, could you check whether the soup has milk?",
            "before we order, could you check whether the sauce has nuts?",
        ],
    ),
    (
        "could we take a break and get some",
        ["fresh", "water"],
        ["fres", "wate"],
        [
            "could we take a break and get some fresh air?",
            "could we take a break and get some water?",
        ],
    ),
    (
        "please send me a message when you reach the",
        ["station", "hotel"],
        ["statio", "hote"],
        [
            "please send me a message when you reach the station.",
            "please send me a message when you reach the hotel.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "qu'jian'ping'zheng",
        "qu'jian'ping'zhen",
        "取件凭证",
        [
            "取件凭证我已经保存，等会儿去柜台出示。",
            "取件凭证没有收到，可以麻烦你再发一次吗？",
        ],
    ),
    (
        "yu'yue'ping'zheng",
        "yu'yue'ping'zhen",
        "预约凭证",
        [
            "预约凭证我已经保存好了，到现场时可以出示。",
            "预约凭证找不到了，可以帮我重新发送一份吗？",
        ],
    ),
    (
        "bao'xiu'ji'lu",
        "bao'xiu'ji'l",
        "报修记录",
        [
            "报修记录里写了设备的问题，麻烦先看一下。",
            "报修记录可以在哪里查看？我想确认上门时间。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "funshitsutodoke",
        "funshitsutodok",
        "紛失届",
        [
            "紛失届を出したいのですが、どこで手続きできますか。",
            "紛失届に必要な情報を確認してから、窓口へ行きます。",
        ],
    ),
    (
        "uketsukejikan",
        "uketsukejika",
        "受付時間",
        [
            "受付時間は何時から何時までですか。",
            "受付時間に間に合わない場合は、明日伺います。",
        ],
    ),
    (
        "fuguai",
        "fugua",
        "不具合",
        [
            "不具合が出たときの状況を、写真と一緒に送ります。",
            "不具合について相談したいので、連絡先を教えてください。",
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
fn inverse_restores_exact_baseline_and_rejects_changed_append() {
    for (raw, first, sentence) in [
        (
            include_str!("../data/lexicons/en.json"),
            EN[0].0,
            EN[0].3[1],
        ),
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            ZH[0].3[0],
            ZH[0].3[1],
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            JA[0].3[0],
            JA[0].3[1],
        ),
    ] {
        let previous = daily_support_resource::without_daily_support(raw);
        assert!(!previous.contains(sentence));
        assert_eq!(
            daily_support_resource::without_daily_support(&previous),
            previous
        );
        for needle in [first, sentence] {
            let mutated = raw.replacen(needle, "unexpected changed entry", 1);
            assert_ne!(mutated, raw);
            assert!(
                std::panic::catch_unwind(|| daily_support_resource::without_daily_support(
                    &mutated
                ))
                .is_err()
            );
        }
    }
}

#[test]
fn english_branches_are_selectable_and_commit_once() {
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
fn cjk_branches_are_selectable_and_commit_once() {
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
fn tones_and_kana_keep_candidates() {
    for (i, seed) in [
        "qu3'jian4'ping2'zheng4",
        "yu4'yue1'ping2'zheng4",
        "bao4'xiu1'ji4'lu4",
    ]
    .into_iter()
    .enumerate()
    {
        contains("zh-Hans", seed, ZH[i].2, CandidateKind::Word);
        for author in ZH[i].3 {
            contains("zh-Hans", seed, author, CandidateKind::Sentence);
        }
    }
    for (i, seed) in ["ふんしつとどけ", "うけつけじかん", "ふぐあい"]
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
fn recommended_packs_preserve_old_and_new_candidates() {
    SUITE.private_packs(
        "recommended_packs_preserve_old_and_new_candidates",
        "SUZAKU_DAILY_SUPPORT_TEST_CHILD",
        |language| match language {
            "en" => contains(language, "reply", "reply", CandidateKind::Word),
            "zh-Hans" => contains(
                language,
                "xia'zai'lian'jie",
                "下载链接",
                CandidateKind::Word,
            ),
            "ja" => contains(language, "tenpufairu", "添付ファイル", CandidateKind::Word),
            _ => unreachable!(),
        },
        |language| match language {
            "en" => {
                for &(context, words, _, authors) in EN {
                    for i in 0..2 {
                        contains(
                            language,
                            &format!("{context} {}", words[i]),
                            authors[i],
                            CandidateKind::Sentence,
                        );
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
        "daily-support-unavailable"
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
fn unavailable_model_retains_local_candidates() {
    for (language, seed, author) in [
        (
            "en",
            "before we order, could you check whether the soup",
            EN[0].3[0],
        ),
        ("zh-Hans", ZH[0].0, ZH[0].3[0]),
        ("ja", JA[0].0, JA[0].3[0]),
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

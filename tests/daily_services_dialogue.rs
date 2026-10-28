//! Household and daily-service candidates work without model availability.
#[allow(dead_code)]
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/daily_services_resource.rs"]
mod daily_services_resource;

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
    marker: "DAILY_SERVICES",
};
const EN: &[EnglishCase] = &[
    (
        "before we leave, could you check the",
        ["front", "back"],
        ["fron", "bac"],
        [
            "before we leave, could you check the front door is locked?",
            "before we leave, could you check the back window is closed?",
        ],
    ),
    (
        "please remember to bring your",
        ["keys", "phone"],
        ["key", "phon"],
        [
            "please remember to bring your keys when you go out.",
            "please remember to bring your phone in case we need to reach you.",
        ],
    ),
    (
        "are you having dinner",
        ["at", "with"],
        ["a", "wit"],
        [
            "are you having dinner at home tonight?",
            "are you having dinner with us tonight?",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "pei'song",
        "pei'son",
        "配送",
        [
            "配送时间可以改到明天下午吗？今天家里没人。",
            "配送前麻烦给我打个电话，我好下楼取件。",
        ],
    ),
    (
        "fu'wu'tai",
        "fu'wu'ta",
        "服务台",
        [
            "服务台在二楼电梯旁，需要帮助可以去那里问一下。",
            "服务台可以帮忙查询遗失物品吗？我刚才找不到钥匙了。",
        ],
    ),
    (
        "gui'tai",
        "gui'ta",
        "柜台",
        [
            "柜台已经关了，请问明天几点可以办理？",
            "柜台工作人员让我带上身份证，明天再来一趟。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "yoyaku",
        "yoyak",
        "予約",
        [
            "予約の時間を明日の午後に変更できますか。",
            "予約した名前を伝えますので、確認をお願いします。",
        ],
    ),
    (
        "haisou",
        "haiso",
        "配送",
        [
            "配送は明日の午後に変更できますか。今日は家にいません。",
            "配送前に電話をいただけると助かります。",
        ],
    ),
    (
        "uketsuke",
        "uketsuk",
        "受付",
        [
            "受付で番号を受け取ってから、こちらでお待ちください。",
            "受付は何時まで開いていますか。必要な持ち物も教えてください。",
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
            "before we leave, could you check the",
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
        let previous = daily_services_resource::without_daily_services(raw);
        assert!(!previous.contains(sentence));
        assert_eq!(
            daily_services_resource::without_daily_services(&previous),
            previous
        );
        for needle in [first, sentence] {
            let mutated = raw.replacen(needle, "unexpected changed entry", 1);
            assert_ne!(mutated, raw);
            assert!(
                std::panic::catch_unwind(|| daily_services_resource::without_daily_services(
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
fn chinese_tones_and_japanese_kana_keep_candidates() {
    for (i, seed) in ["pei4'song4", "fu2'wu4'tai2", "gui4'tai2"]
        .into_iter()
        .enumerate()
    {
        contains("zh-Hans", seed, ZH[i].2, CandidateKind::Word);
        for author in ZH[i].3 {
            contains("zh-Hans", seed, author, CandidateKind::Sentence);
        }
    }
    for (i, seed) in ["よやく", "はいそう", "うけつけ"].into_iter().enumerate() {
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
        "SUZAKU_DAILY_SERVICES_TEST_CHILD",
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
        "daily-services-unavailable"
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
            "before we leave, could you check the front",
            EN[0].3[0],
        ),
        ("zh-Hans", "pei'song", ZH[0].3[0]),
        ("ja", "yoyaku", JA[0].3[0]),
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

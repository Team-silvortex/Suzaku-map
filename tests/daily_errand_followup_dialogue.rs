//! Daily errand follow-up candidates remain usable when predictions are unavailable.
#[allow(dead_code)]
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/daily_errand_followup_resource.rs"]
mod daily_errand_followup_resource;

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
    marker: "DAILY_ERRAND_FOLLOWUP",
};
const EN: &[EnglishCase] = &[
    (
        "when the weather is clear, we can",
        ["walk", "leave"],
        ["wal", "leav"],
        [
            "when the weather is clear, we can walk to the station.",
            "when the weather is clear, we can leave for the park.",
        ],
    ),
    (
        "please put the receipt in the",
        ["bag", "drawer"],
        ["ba", "drawe"],
        [
            "please put the receipt in the bag with the items.",
            "please put the receipt in the drawer with the keys.",
        ],
    ),
    (
        "i'll charge my phone before",
        ["leaving", "dinner"],
        ["leavin", "dinne"],
        [
            "i'll charge my phone before leaving for the station.",
            "i'll charge my phone before dinner so it is ready later.",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "shen'qing'biao",
        "shen'qing'bia",
        "申请表",
        [
            "申请表填好后请先检查联系方式，再提交给工作人员。",
            "申请表需要附上哪些材料？我想提前准备好。",
        ],
    ),
    (
        "bei'yong'dian'chi",
        "bei'yong'dian'ch",
        "备用电池",
        [
            "备用电池放在抽屉里，出门前记得带上。",
            "备用电池还有电吗？我先检查一下再装进去。",
        ],
    ),
    (
        "ting'che'chang",
        "ting'che'chan",
        "停车场",
        [
            "停车场的入口在大楼后面，沿着指示牌走就能看到。",
            "停车场的缴费方式是什么？我想先确认一下。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "shinseisho",
        "shinseish",
        "申請書",
        [
            "申請書を書き終えたら、連絡先を確認して提出します。",
            "申請書と一緒に必要な書類を教えていただけますか。",
        ],
    ),
    (
        "okurijou",
        "okurijo",
        "送り状",
        [
            "送り状に住所を書いたら、発送前にもう一度確認します。",
            "送り状の番号を教えていただけますか。配送状況を確認します。",
        ],
    ),
    (
        "junjo",
        "junj",
        "順序",
        [
            "順序を確認してから、一つずつ作業を進めます。",
            "順序が変わる場合は、先に知らせていただけると助かります。",
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
        let previous = daily_errand_followup_resource::without_daily_errand_followup(raw);
        assert!(!previous.contains(sentence));
        assert_eq!(
            daily_errand_followup_resource::without_daily_errand_followup(&previous),
            previous
        );
        for needle in [first, sentence] {
            let mutated = raw.replacen(needle, "unexpected changed entry", 1);
            assert_ne!(mutated, raw);
            assert!(
                std::panic::catch_unwind(|| {
                    daily_errand_followup_resource::without_daily_errand_followup(&mutated)
                })
                .is_err()
            );
        }
    }
    // Later additions must be guarded just as strictly as the first marker.
    for (raw, needles) in [
        (
            include_str!("../data/lexicons/en.json"),
            [EN[1].0, EN[2].3[1]],
        ),
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            [ZH[1].3[0], ZH[2].3[1]],
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            [JA[1].3[0], JA[2].3[1]],
        ),
    ] {
        for needle in needles {
            let mutated = raw.replacen(needle, "unexpected changed entry", 1);
            assert_ne!(mutated, raw);
            assert!(
                std::panic::catch_unwind(|| {
                    daily_errand_followup_resource::without_daily_errand_followup(&mutated)
                })
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
        "shen1'qing3'biao3",
        "bei4'yong4'dian4'chi2",
        "ting2'che1'chang3",
    ]
    .into_iter()
    .enumerate()
    {
        contains("zh-Hans", seed, ZH[i].2, CandidateKind::Word);
        for author in ZH[i].3 {
            contains("zh-Hans", seed, author, CandidateKind::Sentence);
        }
    }
    for (i, seed) in ["しんせいしょ", "おくりじょう", "じゅんじょ"]
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
        "SUZAKU_DAILY_ERRAND_FOLLOWUP_TEST_CHILD",
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
        "daily-errand-followup-unavailable"
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
        ("en", "when the weather is clear, we can walk", EN[0].3[0]),
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

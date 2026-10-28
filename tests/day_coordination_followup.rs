//! Daily coordination retains local candidates without a prediction provider.
#[allow(dead_code)]
#[path = "support/daily_dialogue.rs"]
mod daily_dialogue;
#[path = "support/day_coordination_resource.rs"]
mod day_coordination_resource;

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
    marker: "DAY_COORDINATION_FOLLOWUP",
};
const EN: &[EnglishCase] = &[
    (
        "i'm free for a quick call",
        ["after", "before"],
        ["afte", "befor"],
        [
            "i'm free for a quick call after lunch if you have time.",
            "i'm free for a quick call before the meeting starts.",
        ],
    ),
    (
        "let's leave a little extra time for",
        ["questions", "travel"],
        ["question", "trave"],
        [
            "let's leave a little extra time for questions at the end.",
            "let's leave a little extra time for travel in case there is traffic.",
        ],
    ),
    (
        "if the link does not work, please",
        ["send", "tell"],
        ["sen", "tel"],
        [
            "if the link does not work, please send me another one.",
            "if the link does not work, please tell me what you see on the screen.",
        ],
    ),
    (
        "would you prefer to meet",
        ["before", "after"],
        ["befor", "afte"],
        [
            "would you prefer to meet before lunch tomorrow?",
            "would you prefer to meet after work on friday?",
        ],
    ),
];
const ZH: &[ReadingCase] = &[
    (
        "ri'cheng",
        "ri'chen",
        "日程",
        [
            "日程我已经记下了，如果时间有变化，请提前告诉我。",
            "日程里还有一处时间冲突，我们先确认哪一项需要调整。",
        ],
    ),
    (
        "ji'he'dian",
        "ji'he'dia",
        "集合点",
        [
            "集合点改到车站东门，到了以后在群里说一声。",
            "集合点附近有一家便利店，我们可以在那里等。",
        ],
    ),
    (
        "yu'ji'dao'da",
        "yu'ji'dao'd",
        "预计到达",
        [
            "预计到达时间是下午两点，路上如果堵车我会及时告诉你。",
            "预计到达时间还不确定，我出发后再发消息确认。",
        ],
    ),
    (
        "ti'xing'yi'xia",
        "ti'xing'yi'xi",
        "提醒一下",
        [
            "提醒一下，明天出门前记得带上证件和预约凭证。",
            "提醒一下，这份材料需要在今天下班前发给对方。",
        ],
    ),
];
const JA: &[ReadingCase] = &[
    (
        "henshinmachi",
        "henshinmach",
        "返信待ち",
        [
            "返信待ちのため、予定が決まり次第お知らせします。",
            "返信待ちの間に、先に必要な資料をそろえておきます。",
        ],
    ),
    (
        "yoyakukakunin",
        "yoyakukakuni",
        "予約確認",
        [
            "予約確認のメールが届いているか、出発前に見ておきます。",
            "予約確認をお願いしたいのですが、名前をお伝えすればよいですか。",
        ],
    ),
    (
        "gouryuuchiten",
        "gouryuuchite",
        "合流地点",
        [
            "合流地点は駅の東口にしましょう。着いたら連絡してください。",
            "合流地点を変える場合は、先に地図の位置を送ってください。",
        ],
    ),
    (
        "enchou",
        "encho",
        "延長",
        [
            "延長をお願いしたいのですが、いつまで借りられますか。",
            "延長が難しい場合は、予定どおり明日返却します。",
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
    for (raw, first, authors) in [
        (
            include_str!("../data/lexicons/en.json"),
            EN[0].0,
            EN.iter().map(|c| c.3[1]).collect::<Vec<_>>(),
        ),
        (
            include_str!("../data/lexicons/zh-Hans.json"),
            ZH[0].3[0],
            ZH.iter().map(|c| c.3[1]).collect(),
        ),
        (
            include_str!("../data/lexicons/ja.json"),
            JA[0].3[0],
            JA.iter().map(|c| c.3[1]).collect(),
        ),
    ] {
        let previous = day_coordination_resource::without_day_coordination(raw);
        for author in &authors {
            assert!(!previous.contains(author));
        }
        assert_eq!(
            day_coordination_resource::without_day_coordination(&previous),
            previous
        );
        for needle in std::iter::once(first).chain(authors) {
            let mutated = raw.replacen(needle, "unexpected changed entry", 1);
            assert_ne!(mutated, raw);
            assert!(
                std::panic::catch_unwind(|| {
                    day_coordination_resource::without_day_coordination(&mutated)
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
        "ri4 cheng2",
        "ji2 he2 dian3",
        "yu4 ji4 dao4 da2",
        "ti2 xing3 yi2 xia4",
    ]
    .into_iter()
    .enumerate()
    {
        contains("zh-Hans", seed, ZH[i].2, CandidateKind::Word);
        for author in ZH[i].3 {
            contains("zh-Hans", seed, author, CandidateKind::Sentence);
        }
    }
    for (i, seed) in [
        "へんしんまち",
        "よやくかくにん",
        "ごうりゅうちてん",
        "えんちょう",
    ]
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
        "SUZAKU_DAY_COORDINATION_FOLLOWUP_TEST_CHILD",
        |language| match language {
            "en" => contains(language, "reply", "reply", CandidateKind::Word),
            "zh-Hans" => contains(language, "shen'qing'biao", "申请表", CandidateKind::Word),
            "ja" => contains(language, "shinseisho", "申請書", CandidateKind::Word),
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
        "day-coordination-followup-unavailable"
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
        ("en", "i'm free for a quick call after", EN[0].3[0]),
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

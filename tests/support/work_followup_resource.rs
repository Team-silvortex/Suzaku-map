//! Exact test-only inverse of the finite work follow-up append.
use ring::digest::{SHA256, digest};
#[path = "work_access_resource.rs"]
mod work_access_resource;

fn sha(raw: &str) -> String {
    digest(&SHA256, raw.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn remove_once(raw: &mut String, block: &str) {
    assert_eq!(
        raw.matches(block).count(),
        1,
        "exact appended bytes changed"
    );
    let start = raw.find(block).unwrap();
    raw.replace_range(start..start + block.len(), "");
}

pub fn without_work_followup(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "2fd18c34ae5ad4074e9963237487383d1144d8ef98d85c4e2a66985b2987f074",
        "zh-Hans" => "b63de98f1f837a4ae666509e6159b0d84b498760565db654ba7b3f330fa39c3f",
        "ja" => "fa8d2d92bcf7cd69c288d0e3bf7b6d553a7bc87da80a7ab7eb1e56524707d0c2",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let restored = work_access_resource::without_work_access(raw);
    let raw = restored.as_str();
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let mut previous = raw.to_owned();
    match language {
        "en" => {
            let layers = value["word_layers"].as_array().unwrap();
            assert_eq!(layers.len(), 64);
            let layer = &layers[63];
            assert_eq!(layer["id"], "daily_presence_dialogue");
            assert_eq!(layer["words"], serde_json::json!([]));
            let contexts = layer["next_words"].as_array().unwrap();
            let authors = layer["sentences"].as_array().unwrap();
            assert_eq!(contexts.len(), 37);
            assert_eq!(authors.len(), 74);
            assert_eq!(
                &contexts[34..],
                serde_json::json!([
                    ["after the meeting, i'll", ["send", "update"]],
                    ["if the deadline changes, please", ["tell", "let"]],
                    ["here is a quick update on", ["what", "where"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                &authors[68..],
                serde_json::json!([
                    "after the meeting, i'll send the action items to everyone.",
                    "after the meeting, i'll update the action items with the owners.",
                    "if the deadline changes, please tell me as soon as you know.",
                    "if the deadline changes, please let the team know before the end of the day.",
                    "here is a quick update on what is done and what still needs work.",
                    "here is a quick update on where we are and what comes next."
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
        [
          "after the meeting, i'll",
          [
            "send",
            "update"
          ]
        ],
        [
          "if the deadline changes, please",
          [
            "tell",
            "let"
          ]
        ],
        [
          "here is a quick update on",
          [
            "what",
            "where"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "after the meeting, i'll send the action items to everyone.",
        "after the meeting, i'll update the action items with the owners.",
        "if the deadline changes, please tell me as soon as you know.",
        "if the deadline changes, please let the team know before the end of the day.",
        "here is a quick update on what is done and what still needs work.",
        "here is a quick update on where we are and what comes next.""#,
            );
        }
        "zh-Hans" => {
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(contexts.len(), 943);
            assert_eq!(
                &contexts[940..],
                serde_json::json!([
                    [
                        "期限",
                        [
                            "期限可能赶不上，我今天会先说明原因和预计完成时间。",
                            "期限可以调整到下周吗？我会先整理好已完成的部分。"
                        ]
                    ],
                    [
                        "截止日期",
                        [
                            "截止日期是周五中午，我会提前把材料发给你。",
                            "截止日期如果有变化，请及时告诉我，我好调整安排。"
                        ]
                    ],
                    [
                        "最新进展",
                        [
                            "最新进展是主要内容已经完成，剩下的问题正在核对。",
                            "最新进展我会在今天下班前发到群里。"
                        ]
                    ]
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
    [
      "期限",
      [
        "期限可能赶不上，我今天会先说明原因和预计完成时间。",
        "期限可以调整到下周吗？我会先整理好已完成的部分。"
      ]
    ],
    [
      "截止日期",
      [
        "截止日期是周五中午，我会提前把材料发给你。",
        "截止日期如果有变化，请及时告诉我，我好调整安排。"
      ]
    ],
    [
      "最新进展",
      [
        "最新进展是主要内容已经完成，剩下的问题正在核对。",
        "最新进展我会在今天下班前发到群里。"
      ]
    ]"#,
            );
        }
        "ja" => {
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(contexts.len(), 431);
            assert_eq!(
                &contexts[428..],
                serde_json::json!([
                    [
                        "進捗",
                        [
                            "進捗を共有します。作業は予定どおり進んでいます。",
                            "進捗に変化があれば、分かった時点でお知らせします。"
                        ]
                    ],
                    [
                        "引き継ぎ",
                        [
                            "引き継ぎの前に、未完了の作業と担当者をまとめます。",
                            "引き継ぎに必要な資料は、明日までに共有します。"
                        ]
                    ],
                    [
                        "相談",
                        [
                            "相談したいことがあります。納期について少しお時間をいただけますか。",
                            "相談の結果、作業の優先順位を見直すことになりました。"
                        ]
                    ]
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
    [
      "進捗",
      [
        "進捗を共有します。作業は予定どおり進んでいます。",
        "進捗に変化があれば、分かった時点でお知らせします。"
      ]
    ],
    [
      "引き継ぎ",
      [
        "引き継ぎの前に、未完了の作業と担当者をまとめます。",
        "引き継ぎに必要な資料は、明日までに共有します。"
      ]
    ],
    [
      "相談",
      [
        "相談したいことがあります。納期について少しお時間をいただけますか。",
        "相談の結果、作業の優先順位を見直すことになりました。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire pre-work-followup resource changed"
    );
    previous
}

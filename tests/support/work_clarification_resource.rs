//! Exact test-only inverse of the finite work_clarification append.
use ring::digest::{SHA256, digest};
#[path = "work_followup_resource.rs"]
mod work_followup_resource;

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

pub fn without_work_clarification(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "8cafe0c4c7f47de2db83ec588a6399442e3fbab0bbc5cde43d088662a218c0fb",
        "zh-Hans" => "1f569d38223393870c8c6b6f41a817f4c068126eae2b001d43533f97ee0ee288",
        "ja" => "cb99141e60f2ada2f5f1fc2ff557abec456053905ea4289ad6ce49cd0c3992d3",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let previous = work_followup_resource::without_work_followup(raw);
    let raw = previous.as_str();
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
            assert_eq!(contexts.len(), 34);
            assert_eq!(authors.len(), 68);
            assert_eq!(
                &contexts[31..],
                serde_json::json!([
                    [
                        "before i make the change, could you confirm",
                        ["which", "what"]
                    ],
                    ["about your feedback, could you", ["show", "explain"]],
                    ["reply when you've had time to", ["check", "finish"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(&authors[62..], serde_json::json!(["before i make the change, could you confirm which version we should use?", "before i make the change, could you confirm what needs to stay the same?", "about your feedback, could you show me which part needs work?", "about your feedback, could you explain why you would prefer a different option?", "reply when you've had time to check the details with the team.", "reply when you've had time to finish what you're working on."]).as_array().unwrap());
            remove_once(
                &mut previous,
                r#",
        [
          "before i make the change, could you confirm",
          [
            "which",
            "what"
          ]
        ],
        [
          "about your feedback, could you",
          [
            "show",
            "explain"
          ]
        ],
        [
          "reply when you've had time to",
          [
            "check",
            "finish"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "before i make the change, could you confirm which version we should use?",
        "before i make the change, could you confirm what needs to stay the same?",
        "about your feedback, could you show me which part needs work?",
        "about your feedback, could you explain why you would prefer a different option?",
        "reply when you've had time to check the details with the team.",
        "reply when you've had time to finish what you're working on.""#,
            );
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(contexts.len(), 940);
            assert_eq!(
                &contexts[937..],
                serde_json::json!([
                    [
                        "待确认",
                        [
                            "待确认的地方我已经标出来了，麻烦你方便时看一下。",
                            "待确认的问题先保留，核对清楚后我们再继续。"
                        ]
                    ],
                    [
                        "修改意见",
                        [
                            "修改意见可以具体到段落吗？这样我比较容易定位。",
                            "修改意见里请说明哪里要改，哪些内容需要保持原样。"
                        ]
                    ],
                    [
                        "不急着回复",
                        [
                            "不急着回复，先把手头的事情忙完就好。",
                            "不急着回复，等你和同事核对清楚后再告诉我。"
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
      "待确认",
      [
        "待确认的地方我已经标出来了，麻烦你方便时看一下。",
        "待确认的问题先保留，核对清楚后我们再继续。"
      ]
    ],
    [
      "修改意见",
      [
        "修改意见可以具体到段落吗？这样我比较容易定位。",
        "修改意见里请说明哪里要改，哪些内容需要保持原样。"
      ]
    ],
    [
      "不急着回复",
      [
        "不急着回复，先把手头的事情忙完就好。",
        "不急着回复，等你和同事核对清楚后再告诉我。"
      ]
    ]"#,
            );
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            assert_eq!(contexts.len(), 428);
            assert_eq!(
                &contexts[425..],
                serde_json::json!([
                    [
                        "役割",
                        [
                            "役割を分ける前に、誰が何を担当するか確認しましょう。",
                            "役割が重なっているところは、一度整理してから進めませんか。"
                        ]
                    ],
                    [
                        "用語",
                        [
                            "用語の意味が少し曖昧なので、具体例を一つ教えてください。",
                            "用語の使い方をそろえるために、この資料の説明を確認しましょう。"
                        ]
                    ],
                    [
                        "情報",
                        [
                            "情報を確認してからで大丈夫です。分かったところだけ返事をください。",
                            "情報がまだ足りなければ、返事は明日でも構いません。"
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
      "役割",
      [
        "役割を分ける前に、誰が何を担当するか確認しましょう。",
        "役割が重なっているところは、一度整理してから進めませんか。"
      ]
    ],
    [
      "用語",
      [
        "用語の意味が少し曖昧なので、具体例を一つ教えてください。",
        "用語の使い方をそろえるために、この資料の説明を確認しましょう。"
      ]
    ],
    [
      "情報",
      [
        "情報を確認してからで大丈夫です。分かったところだけ返事をください。",
        "情報がまだ足りなければ、返事は明日でも構いません。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire pre-work_clarification resource changed"
    );
    previous
}

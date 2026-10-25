//! Exact test-only inverse of the finite alternatives append.
use ring::digest::{SHA256, digest};
#[path = "rescheduling_resource.rs"]
mod rescheduling_resource;
pub use rescheduling_resource::without_rescheduling;

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

pub fn without_alternatives(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "95e1af47438a06ed7ee9e03209eb1248b6cf18c225e2ffb9cf89ec5098a4e579",
        "zh-Hans" => "7acb1698962a1a6720793bde15d3766f96d1d6fbe8d0040914939e0800980bbe",
        "ja" => "3e70cd7f7ed26dfd1c38d5e5fc889d3f1fcca5af761f3894b9a47acbf407468d",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let previous = without_rescheduling(raw);
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
            assert_eq!(contexts.len(), 12);
            assert_eq!(authors.len(), 24);
            assert_eq!(
                &contexts[9..],
                serde_json::json!([
                    ["if that does not work, we could", ["try", "wait"]],
                    ["would you be happy with", ["tea", "coffee"]],
                    ["we could go with the", ["first", "second"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                &authors[18..],
                serde_json::json!([
                    "if that does not work, we could try a different place.",
                    "if that does not work, we could wait until tomorrow.",
                    "would you be happy with tea instead of coffee?",
                    "would you be happy with coffee while we wait?",
                    "we could go with the first option if that works for you.",
                    "we could go with the second option and leave a little later."
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
        [
          "if that does not work, we could",
          [
            "try",
            "wait"
          ]
        ],
        [
          "would you be happy with",
          [
            "tea",
            "coffee"
          ]
        ],
        [
          "we could go with the",
          [
            "first",
            "second"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "if that does not work, we could try a different place.",
        "if that does not work, we could wait until tomorrow.",
        "would you be happy with tea instead of coffee?",
        "would you be happy with coffee while we wait?",
        "we could go with the first option if that works for you.",
        "we could go with the second option and leave a little later.""#,
            );
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(contexts.len(), 918);
            assert_eq!(
                &contexts[915..],
                serde_json::json!([
                    [
                        "备选",
                        [
                            "备选方案先留着，等大家确认后再决定。",
                            "备选的时间也发给我吧。"
                        ]
                    ],
                    ["替代", ["替代方案还有别的吗？", "替代的材料要提前准备好。"]],
                    [
                        "商量",
                        [
                            "商量一下再决定，大家都说说想法吧。",
                            "商量好了，我再告诉你。"
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
      "备选",
      [
        "备选方案先留着，等大家确认后再决定。",
        "备选的时间也发给我吧。"
      ]
    ],
    [
      "替代",
      [
        "替代方案还有别的吗？",
        "替代的材料要提前准备好。"
      ]
    ],
    [
      "商量",
      [
        "商量一下再决定，大家都说说想法吧。",
        "商量好了，我再告诉你。"
      ]
    ]"#,
            );
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            assert_eq!(contexts.len(), 406);
            assert_eq!(
                &contexts[403..],
                serde_json::json!([
                    [
                        "好き",
                        [
                            "好きな方を選んでください。",
                            "好きな色があれば教えてください。"
                        ]
                    ],
                    [
                        "片方",
                        [
                            "片方だけ先に試してもいいですか。",
                            "片方を選ぶなら、こちらが使いやすそうです。"
                        ]
                    ],
                    [
                        "決める",
                        [
                            "決める前に、みんなの希望も聞きたいです。",
                            "決めるのは明日でも大丈夫です。"
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
      "好き",
      [
        "好きな方を選んでください。",
        "好きな色があれば教えてください。"
      ]
    ],
    [
      "片方",
      [
        "片方だけ先に試してもいいですか。",
        "片方を選ぶなら、こちらが使いやすそうです。"
      ]
    ],
    [
      "決める",
      [
        "決める前に、みんなの希望も聞きたいです。",
        "決めるのは明日でも大丈夫です。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire rephrasing resource changed"
    );
    previous
}

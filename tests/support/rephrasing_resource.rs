//! Exact test-only inverse of the finite rephrasing append.
use ring::digest::{SHA256, digest};
#[path = "alternatives_resource.rs"]
mod alternatives_resource;
pub use alternatives_resource::without_alternatives;

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

pub fn without_rephrasing(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "aef47b5fdab0f0af87dc1773f2fdc295b25412083157925967bbcbb78b367677",
        "zh-Hans" => "44c141800c25846b4038d9361408648849cb68eb6c574df1fc1e50dd255b8efd",
        "ja" => "b8deee7e84006ecbd0b55436c2ac9be55ccc93bbee6f731f634f2c7fbfc61b1e",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let previous = without_alternatives(raw);
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
            assert_eq!(contexts.len(), 9);
            assert_eq!(authors.len(), 18);
            assert_eq!(
                &contexts[6..],
                serde_json::json!([
                    ["actually, i meant", ["tomorrow", "tonight"]],
                    ["let me put that a little more", ["clearly", "simply"]],
                    ["could we check the", ["details", "times"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                &authors[12..],
                serde_json::json!([
                    "actually, i meant tomorrow morning, not today.",
                    "actually, i meant tonight after dinner, not tomorrow.",
                    "let me put that a little more clearly: we can start after lunch.",
                    "let me put that a little more simply: we should wait here.",
                    "could we check the details before we leave?",
                    "could we check the times again before we make plans?"
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
        [
          "actually, i meant",
          [
            "tomorrow",
            "tonight"
          ]
        ],
        [
          "let me put that a little more",
          [
            "clearly",
            "simply"
          ]
        ],
        [
          "could we check the",
          [
            "details",
            "times"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "actually, i meant tomorrow morning, not today.",
        "actually, i meant tonight after dinner, not tomorrow.",
        "let me put that a little more clearly: we can start after lunch.",
        "let me put that a little more simply: we should wait here.",
        "could we check the details before we leave?",
        "could we check the times again before we make plans?""#,
            );
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(contexts.len(), 915);
            assert_eq!(
                &contexts[912..],
                serde_json::json!([
                    [
                        "记错了",
                        [
                            "记错了，刚才说的应该是明天下午。",
                            "记错了，不是三点，是四点。"
                        ]
                    ],
                    [
                        "换一种说法",
                        [
                            "换一种说法，我是想先听听大家的意见。",
                            "换一种说法，我们先把时间定下来，再选地方。"
                        ]
                    ],
                    [
                        "暂不确定",
                        [
                            "暂不确定，我想先核对一下再回复你。",
                            "暂不确定能不能赶上，晚点给你准信。"
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
      "记错了",
      [
        "记错了，刚才说的应该是明天下午。",
        "记错了，不是三点，是四点。"
      ]
    ],
    [
      "换一种说法",
      [
        "换一种说法，我是想先听听大家的意见。",
        "换一种说法，我们先把时间定下来，再选地方。"
      ]
    ],
    [
      "暂不确定",
      [
        "暂不确定，我想先核对一下再回复你。",
        "暂不确定能不能赶上，晚点给你准信。"
      ]
    ]"#,
            );
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            assert_eq!(contexts.len(), 403);
            assert_eq!(
                &contexts[400..],
                serde_json::json!([
                    [
                        "要約",
                        [
                            "要約すると、先に時間を決めてから場所を選びます。",
                            "要約すると、今日は予定を確認するだけです。"
                        ]
                    ],
                    [
                        "詳細",
                        [
                            "詳細はまだ分からないので、確認してからお伝えします。",
                            "詳細がはっきりするまでは、返事を少し待ってください。"
                        ]
                    ],
                    [
                        "事実",
                        [
                            "事実と違っていたので、先ほどの説明を訂正します。",
                            "事実を確認したところ、会う日は明日でした。"
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
      "要約",
      [
        "要約すると、先に時間を決めてから場所を選びます。",
        "要約すると、今日は予定を確認するだけです。"
      ]
    ],
    [
      "詳細",
      [
        "詳細はまだ分からないので、確認してからお伝えします。",
        "詳細がはっきりするまでは、返事を少し待ってください。"
      ]
    ],
    [
      "事実",
      [
        "事実と違っていたので、先ほどの説明を訂正します。",
        "事実を確認したところ、会う日は明日でした。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire turntaking resource changed"
    );
    previous
}

//! Exact test-only inverse of the finite rescheduling append.
use ring::digest::{SHA256, digest};
#[path = "consideration_resource.rs"]
mod consideration_resource;

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

pub fn without_rescheduling(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "e48baa0c2b12dc3b024082834f5bc452f0afa201fda68cf290a66057b89b58ac",
        "zh-Hans" => "093e752b7d626d21f46342ba7446a238cf3a0b8ac934f469564f0da6d46bb4ae",
        "ja" => "b53c371d6fe37ce983f72c9cdd92c975173e746c12021219c40b48a27400fc19",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let previous = consideration_resource::without_consideration(raw);
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
            assert_eq!(contexts.len(), 15);
            assert_eq!(authors.len(), 30);
            assert_eq!(
                &contexts[12..],
                serde_json::json!([
                    ["something came up, could we meet", ["tomorrow", "later"]],
                    ["if you are running late, we can", ["wait", "start"]],
                    ["let's keep the same place and change the", ["time", "day"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                &authors[24..],
                serde_json::json!([
                    "something came up, could we meet tomorrow at the same time?",
                    "something came up, could we meet later this week?",
                    "if you are running late, we can wait here for a few minutes.",
                    "if you are running late, we can start a little later.",
                    "let's keep the same place and change the time to three instead.",
                    "let's keep the same place and change the day to friday."
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
        [
          "something came up, could we meet",
          [
            "tomorrow",
            "later"
          ]
        ],
        [
          "if you are running late, we can",
          [
            "wait",
            "start"
          ]
        ],
        [
          "let's keep the same place and change the",
          [
            "time",
            "day"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "something came up, could we meet tomorrow at the same time?",
        "something came up, could we meet later this week?",
        "if you are running late, we can wait here for a few minutes.",
        "if you are running late, we can start a little later.",
        "let's keep the same place and change the time to three instead.",
        "let's keep the same place and change the day to friday.""#,
            );
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(contexts.len(), 921);
            assert_eq!(
                &contexts[918..],
                serde_json::json!([
                    [
                        "临时有安排",
                        [
                            "临时有安排，今天可能没法过去了。",
                            "临时有安排，我们能换个时间吗？"
                        ]
                    ],
                    [
                        "改个时间吧",
                        [
                            "改个时间吧，明天下午你方便吗？",
                            "改个时间吧，我今天临时有事。"
                        ]
                    ],
                    [
                        "改约时间",
                        [
                            "改约时间确定后，麻烦再发我一次。",
                            "改约时间先别定，我再确认一下明天的安排。"
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
      "临时有安排",
      [
        "临时有安排，今天可能没法过去了。",
        "临时有安排，我们能换个时间吗？"
      ]
    ],
    [
      "改个时间吧",
      [
        "改个时间吧，明天下午你方便吗？",
        "改个时间吧，我今天临时有事。"
      ]
    ],
    [
      "改约时间",
      [
        "改约时间确定后，麻烦再发我一次。",
        "改约时间先别定，我再确认一下明天的安排。"
      ]
    ]"#,
            );
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            assert_eq!(contexts.len(), 409);
            assert_eq!(
                &contexts[406..],
                serde_json::json!([
                    [
                        "来週",
                        [
                            "来週なら、今のところ火曜日が空いています。",
                            "来週に変更しても大丈夫ですか。"
                        ]
                    ],
                    [
                        "日程",
                        [
                            "日程を調整したいので、都合のよい日を教えてください。",
                            "日程が変わったら、早めに連絡します。"
                        ]
                    ],
                    [
                        "調整",
                        [
                            "調整が必要なら、別の時間も考えましょう。",
                            "調整できたら、改めて連絡します。"
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
      "来週",
      [
        "来週なら、今のところ火曜日が空いています。",
        "来週に変更しても大丈夫ですか。"
      ]
    ],
    [
      "日程",
      [
        "日程を調整したいので、都合のよい日を教えてください。",
        "日程が変わったら、早めに連絡します。"
      ]
    ],
    [
      "調整",
      [
        "調整が必要なら、別の時間も考えましょう。",
        "調整できたら、改めて連絡します。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire alternatives resource changed"
    );
    previous
}

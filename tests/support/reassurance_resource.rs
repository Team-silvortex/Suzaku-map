//! Exact test-only inverse of the finite reassurance append.
use ring::digest::{SHA256, digest};

#[path = "apology_resource.rs"]
mod apology_resource;

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

pub fn without_reassurance(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "1522f322f693db7d18181221b3312a1ab8b905e2d48c1fbf794626acd3f2faf7",
        "zh-Hans" => "6b2963727cc976b92a8a66ee989fcc95652a8e5a5ff3203e5de01f547bb4624b",
        "ja" => "15ed727f43d55b79794409323a21bfddef900db010a4e67ff7e82b0d5549c827",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let previous = apology_resource::without_apology(raw);
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
            assert_eq!(contexts.len(), 24);
            assert_eq!(authors.len(), 48);
            assert_eq!(
                &contexts[21..],
                serde_json::json!([
                    ["when you get home, send me a", ["message", "text"]],
                    ["did you make it", ["home", "back"]],
                    ["just letting you know we're", ["home", "safe"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                &authors[42..],
                serde_json::json!([
                    "when you get home, send me a message so i know you're safe.",
                    "when you get home, send me a text when you have a moment.",
                    "did you make it home safely after the long trip?",
                    "did you make it back okay, or do you need a hand?",
                    "just letting you know we're home and the trip went well.",
                    "just letting you know we're safe and everyone is fine."
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
        [
          "when you get home, send me a",
          [
            "message",
            "text"
          ]
        ],
        [
          "did you make it",
          [
            "home",
            "back"
          ]
        ],
        [
          "just letting you know we're",
          [
            "home",
            "safe"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "when you get home, send me a message so i know you're safe.",
        "when you get home, send me a text when you have a moment.",
        "did you make it home safely after the long trip?",
        "did you make it back okay, or do you need a hand?",
        "just letting you know we're home and the trip went well.",
        "just letting you know we're safe and everyone is fine.""#,
            );
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(contexts.len(), 930);
            assert_eq!(
                &contexts[927..],
                serde_json::json!([
                    [
                        "别担心",
                        [
                            "别担心，我们可以一件一件慢慢处理。",
                            "别担心，有需要的话我可以陪你一起去。"
                        ]
                    ],
                    [
                        "慢慢来",
                        [
                            "慢慢来，不用一次把所有事情做完。",
                            "慢慢来，等你准备好了我们再继续。"
                        ]
                    ],
                    [
                        "注意身体",
                        [
                            "注意身体，别把自己弄得太累。",
                            "注意身体，忙的时候也记得按时吃饭。"
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
      "别担心",
      [
        "别担心，我们可以一件一件慢慢处理。",
        "别担心，有需要的话我可以陪你一起去。"
      ]
    ],
    [
      "慢慢来",
      [
        "慢慢来，不用一次把所有事情做完。",
        "慢慢来，等你准备好了我们再继续。"
      ]
    ],
    [
      "注意身体",
      [
        "注意身体，别把自己弄得太累。",
        "注意身体，忙的时候也记得按时吃饭。"
      ]
    ]"#,
            );
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            assert_eq!(contexts.len(), 418);
            assert_eq!(
                &contexts[415..],
                serde_json::json!([
                    [
                        "お元気ですか",
                        [
                            "お元気ですか。最近、無理をしていませんか。",
                            "お元気ですか。返事は落ち着いたときで大丈夫です。"
                        ]
                    ],
                    [
                        "お久しぶり",
                        [
                            "お久しぶりです。最近は元気にしていますか。",
                            "お久しぶりです。また近況を聞けたらうれしいです。"
                        ]
                    ],
                    [
                        "居眠り",
                        [
                            "居眠りしそうなので、少し休んでからまた連絡します。",
                            "居眠りしてしまって、返事が遅くなりました。もう起きています。"
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
      "お元気ですか",
      [
        "お元気ですか。最近、無理をしていませんか。",
        "お元気ですか。返事は落ち着いたときで大丈夫です。"
      ]
    ],
    [
      "お久しぶり",
      [
        "お久しぶりです。最近は元気にしていますか。",
        "お久しぶりです。また近況を聞けたらうれしいです。"
      ]
    ],
    [
      "居眠り",
      [
        "居眠りしそうなので、少し休んでからまた連絡します。",
        "居眠りしてしまって、返事が遅くなりました。もう起きています。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire pre-reassurance resource changed"
    );
    previous
}

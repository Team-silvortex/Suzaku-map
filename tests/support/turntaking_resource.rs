//! Exact test-only inverse of the finite turn-taking append.
use ring::digest::{SHA256, digest};
#[path = "rephrasing_resource.rs"]
mod rephrasing_resource;
pub use rephrasing_resource::without_rephrasing;

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

pub fn without_turntaking(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "a054309fd23b14f05f6deac5b79917175ae8b44e9d2b686572b535f41863499e",
        "zh-Hans" => "cebfe206a1ac4aad570f9c4109e85cade046ab2b0c712af730f6e19c1fe68fa2",
        "ja" => "b56ebc4a323618b358649375f47cbbe043a7a6ced0707130f72ddc7e747fefbd",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let previous = without_rephrasing(raw);
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
            assert_eq!(contexts.len(), 6);
            assert_eq!(authors.len(), 12);
            assert_eq!(
                &contexts[3..],
                serde_json::json!([
                    ["are you ready to", ["order", "leave"]],
                    ["if you need a hand with the", ["bags", "dishes"]],
                    ["while we're waiting, we could", ["sit", "walk"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                &authors[6..],
                serde_json::json!([
                    "are you ready to order or do you need another minute?",
                    "are you ready to leave or should we wait a little longer?",
                    "if you need a hand with the bags, i can carry one.",
                    "if you need a hand with the dishes, just let me know.",
                    "while we're waiting, we could sit over there.",
                    "while we're waiting, we could walk around for a few minutes."
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
        [
          "are you ready to",
          [
            "order",
            "leave"
          ]
        ],
        [
          "if you need a hand with the",
          [
            "bags",
            "dishes"
          ]
        ],
        [
          "while we're waiting, we could",
          [
            "sit",
            "walk"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "are you ready to order or do you need another minute?",
        "are you ready to leave or should we wait a little longer?",
        "if you need a hand with the bags, i can carry one.",
        "if you need a hand with the dishes, just let me know.",
        "while we're waiting, we could sit over there.",
        "while we're waiting, we could walk around for a few minutes.""#,
            );
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(contexts.len(), 912);
            assert_eq!(
                &contexts[909..],
                serde_json::json!([
                    [
                        "要是有空",
                        [
                            "要是有空，我想借你两分钟问件事。",
                            "要是有空，能帮我确认一下刚才说的时间吗？"
                        ]
                    ],
                    [
                        "麻烦稍等片刻",
                        [
                            "麻烦稍等片刻，我把刚才的地址再核对一下。",
                            "麻烦稍等片刻，等我记下这一点再接着说。"
                        ]
                    ],
                    [
                        "明白你的意思了",
                        [
                            "明白你的意思了，你是想先确定时间再订地方。",
                            "明白你的意思了，我先说一遍，你听听有没有理解错。"
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
      "要是有空",
      [
        "要是有空，我想借你两分钟问件事。",
        "要是有空，能帮我确认一下刚才说的时间吗？"
      ]
    ],
    [
      "麻烦稍等片刻",
      [
        "麻烦稍等片刻，我把刚才的地址再核对一下。",
        "麻烦稍等片刻，等我记下这一点再接着说。"
      ]
    ],
    [
      "明白你的意思了",
      [
        "明白你的意思了，你是想先确定时间再订地方。",
        "明白你的意思了，我先说一遍，你听听有没有理解错。"
      ]
    ]"#,
            );
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            assert_eq!(contexts.len(), 400);
            assert_eq!(
                &contexts[397..],
                serde_json::json!([
                    [
                        "今から",
                        [
                            "今から少しお話ししても大丈夫ですか。",
                            "今から二分だけ、さっきの話を確認してもいいですか。"
                        ]
                    ],
                    [
                        "少し",
                        [
                            "少しだけお時間をいただいてもいいですか。",
                            "少し待ってください。今の説明をメモしてから続きを聞きたいです。"
                        ]
                    ],
                    [
                        "差し支えなければ",
                        [
                            "差し支えなければ、今ここで一つ確認してもいいですか。",
                            "差し支えなければ、お話しできる時間を教えてください。"
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
      "今から",
      [
        "今から少しお話ししても大丈夫ですか。",
        "今から二分だけ、さっきの話を確認してもいいですか。"
      ]
    ],
    [
      "少し",
      [
        "少しだけお時間をいただいてもいいですか。",
        "少し待ってください。今の説明をメモしてから続きを聞きたいです。"
      ]
    ],
    [
      "差し支えなければ",
      [
        "差し支えなければ、今ここで一つ確認してもいいですか。",
        "差し支えなければ、お話しできる時間を教えてください。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(sha(&previous), expected, "entire presence resource changed");
    previous
}

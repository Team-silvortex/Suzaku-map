//! Exact test-only inverse of the finite apology append.
use ring::digest::{SHA256, digest};

#[path = "work_communication_resource.rs"]
mod work_communication_resource;

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

pub fn without_apology(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "19e94be2d721f78e6740e9d6841ffe451a00d69a25631ef29716c61a3be7ee34",
        "zh-Hans" => "b117bd7ff8fd4c7e5f044131db0c60e9e95fa240210f53333a3fa228bbad48e7",
        "ja" => "8ca1500cb7ff53dd27c542f83587e4022d611c45fdfb7253145b158e55a113a4",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let previous = work_communication_resource::without_work_communication(raw);
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
            assert_eq!(contexts.len(), 27);
            assert_eq!(authors.len(), 54);
            assert_eq!(
                &contexts[24..],
                serde_json::json!([
                    ["sorry for the late reply, i was", ["busy", "away"]],
                    ["thanks for pointing that out, i'll", ["check", "fix"]],
                    ["that was my fault, i'll", ["correct", "explain"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                &authors[48..],
                serde_json::json!([
                    "sorry for the late reply, i was busy with work and missed your message.",
                    "sorry for the late reply, i was away from my phone all afternoon.",
                    "thanks for pointing that out, i'll check the details again.",
                    "thanks for pointing that out, i'll fix that and send it again.",
                    "that was my fault, i'll correct the time and send an update.",
                    "that was my fault, i'll explain what happened so it is clear."
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
        [
          "sorry for the late reply, i was",
          [
            "busy",
            "away"
          ]
        ],
        [
          "thanks for pointing that out, i'll",
          [
            "check",
            "fix"
          ]
        ],
        [
          "that was my fault, i'll",
          [
            "correct",
            "explain"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "sorry for the late reply, i was busy with work and missed your message.",
        "sorry for the late reply, i was away from my phone all afternoon.",
        "thanks for pointing that out, i'll check the details again.",
        "thanks for pointing that out, i'll fix that and send it again.",
        "that was my fault, i'll correct the time and send an update.",
        "that was my fault, i'll explain what happened so it is clear.""#,
            );
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(contexts.len(), 933);
            assert_eq!(
                &contexts[930..],
                serde_json::json!([
                    [
                        "漏看消息",
                        [
                            "漏看消息了，不好意思，我现在就回复你。",
                            "漏看消息是我的疏忽，下次我会留意。"
                        ]
                    ],
                    [
                        "看错了",
                        [
                            "看错了，集合时间是明天下午，不是今天。",
                            "看错了，刚才那条消息不是发给我的。"
                        ]
                    ],
                    [
                        "抱歉让你等了",
                        [
                            "抱歉让你等了，刚才没有看到手机上的消息。",
                            "抱歉让你等了，我刚忙完，现在可以慢慢聊。"
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
      "漏看消息",
      [
        "漏看消息了，不好意思，我现在就回复你。",
        "漏看消息是我的疏忽，下次我会留意。"
      ]
    ],
    [
      "看错了",
      [
        "看错了，集合时间是明天下午，不是今天。",
        "看错了，刚才那条消息不是发给我的。"
      ]
    ],
    [
      "抱歉让你等了",
      [
        "抱歉让你等了，刚才没有看到手机上的消息。",
        "抱歉让你等了，我刚忙完，现在可以慢慢聊。"
      ]
    ]"#,
            );
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            assert_eq!(contexts.len(), 421);
            assert_eq!(
                &contexts[418..],
                serde_json::json!([
                    [
                        "謝罪",
                        [
                            "謝罪します。大事な連絡を見落としていました。",
                            "謝罪します。先ほどは私の説明が間違っていました。"
                        ]
                    ],
                    [
                        "再送",
                        [
                            "再送します。さっきのメッセージが届いていなかったようです。",
                            "再送していただけますか。添付の写真が見当たりません。"
                        ]
                    ],
                    [
                        "お知らせ",
                        [
                            "お知らせが遅くなりました。明日の集合場所は駅の東口です。",
                            "お知らせに一つ訂正があります。開始時間は十時ではなく、十一時です。"
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
      "謝罪",
      [
        "謝罪します。大事な連絡を見落としていました。",
        "謝罪します。先ほどは私の説明が間違っていました。"
      ]
    ],
    [
      "再送",
      [
        "再送します。さっきのメッセージが届いていなかったようです。",
        "再送していただけますか。添付の写真が見当たりません。"
      ]
    ],
    [
      "お知らせ",
      [
        "お知らせが遅くなりました。明日の集合場所は駅の東口です。",
        "お知らせに一つ訂正があります。開始時間は十時ではなく、十一時です。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire pre-apology resource changed"
    );
    previous
}

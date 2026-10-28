//! Exact test-only inverse of the work access and communication append.
use ring::digest::{SHA256, digest};
#[path = "daily_services_resource.rs"]
mod daily_services_resource;

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

pub fn without_work_access(raw: &str) -> String {
    let restored = daily_services_resource::without_daily_services(raw);
    let raw = restored.as_str();
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "7907062ce2f32fc532e8f428615ac4f45babba5ffbc2681c57c16a0ea1be2fb6",
        "zh-Hans" => "4ed626200e76601e285d3f7705b83333cf74e7c067cc37d98645622098884374",
        "ja" => "dcbac45b0fc88e7156338875c74e0b699e971cbab9ed63f91028ffec3ce2d1b7",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
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
            assert_eq!(contexts.len(), 40);
            assert_eq!(authors.len(), 80);
            assert_eq!(
                &contexts[37..],
                serde_json::json!([
                    ["i'm blocked until we receive", ["approval", "feedback"]],
                    ["when you have a moment, could you", ["review", "confirm"]],
                    ["if you need more time, we can", ["adjust", "discuss"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                &authors[74..],
                serde_json::json!([
                    "i'm blocked until we receive approval to move forward.",
                    "i'm blocked until we receive feedback on the latest draft.",
                    "when you have a moment, could you review the updated document?",
                    "when you have a moment, could you confirm that the details are correct?",
                    "if you need more time, we can adjust the schedule together.",
                    "if you need more time, we can discuss what to prioritize."
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
        [
          "i'm blocked until we receive",
          [
            "approval",
            "feedback"
          ]
        ],
        [
          "when you have a moment, could you",
          [
            "review",
            "confirm"
          ]
        ],
        [
          "if you need more time, we can",
          [
            "adjust",
            "discuss"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "i'm blocked until we receive approval to move forward.",
        "i'm blocked until we receive feedback on the latest draft.",
        "when you have a moment, could you review the updated document?",
        "when you have a moment, could you confirm that the details are correct?",
        "if you need more time, we can adjust the schedule together.",
        "if you need more time, we can discuss what to prioritize.""#,
            );
        }
        "zh-Hans" => {
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(contexts.len(), 946);
            assert_eq!(
                &contexts[943..],
                serde_json::json!([
                    [
                        "下载链接",
                        [
                            "下载链接打不开，可以麻烦你再发一次吗？",
                            "下载链接我已经收到了，等下载完成后再确认内容。"
                        ]
                    ],
                    [
                        "权限不足",
                        [
                            "权限不足，我暂时打不开这个文件，能帮我检查一下吗？",
                            "权限不足的话，我可以先查看你发来的预览。"
                        ]
                    ],
                    [
                        "网络断开",
                        [
                            "网络断开了，我重新连接后再把文件发给你。",
                            "网络断开的时候我没听清，能麻烦你重复一下吗？"
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
      "下载链接",
      [
        "下载链接打不开，可以麻烦你再发一次吗？",
        "下载链接我已经收到了，等下载完成后再确认内容。"
      ]
    ],
    [
      "权限不足",
      [
        "权限不足，我暂时打不开这个文件，能帮我检查一下吗？",
        "权限不足的话，我可以先查看你发来的预览。"
      ]
    ],
    [
      "网络断开",
      [
        "网络断开了，我重新连接后再把文件发给你。",
        "网络断开的时候我没听清，能麻烦你重复一下吗？"
      ]
    ]"#,
            );
        }
        "ja" => {
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(contexts.len(), 434);
            assert_eq!(
                &contexts[431..],
                serde_json::json!([
                    [
                        "添付ファイル",
                        [
                            "添付ファイルを受け取りました。内容を確認してからお返事します。",
                            "添付ファイルが見当たらないので、もう一度送っていただけますか。"
                        ]
                    ],
                    [
                        "再送信",
                        [
                            "再送信しました。届いているか確認していただけますか。",
                            "再送信する前に、宛先と添付ファイルを確認します。"
                        ]
                    ],
                    [
                        "通信",
                        [
                            "通信が不安定なので、接続を確認してからもう一度連絡します。",
                            "通信が途切れて最後の部分が聞こえませんでした。もう一度お願いします。"
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
      "添付ファイル",
      [
        "添付ファイルを受け取りました。内容を確認してからお返事します。",
        "添付ファイルが見当たらないので、もう一度送っていただけますか。"
      ]
    ],
    [
      "再送信",
      [
        "再送信しました。届いているか確認していただけますか。",
        "再送信する前に、宛先と添付ファイルを確認します。"
      ]
    ],
    [
      "通信",
      [
        "通信が不安定なので、接続を確認してからもう一度連絡します。",
        "通信が途切れて最後の部分が聞こえませんでした。もう一度お願いします。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire pre-work-access resource changed"
    );
    previous
}

//! Exact test-only inverse of the finite farewell append.
use ring::digest::{SHA256, digest};
#[path = "reassurance_resource.rs"]
mod reassurance_resource;

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

pub fn without_farewell(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "591123aac758c7ccd1e75f711aa582af016188583a1e769e879d1da3ee3fcba6",
        "zh-Hans" => "1cdb49a044a5043e398913cccbfba214d5d06feef0f7ce9a0262ff0dd35f1e66",
        "ja" => "785687c646f1604351c857419a2501796f8d8fcf96e09a9d54cba48bf0477aa1",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let previous = reassurance_resource::without_reassurance(raw);
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
            assert_eq!(contexts.len(), 21);
            assert_eq!(authors.len(), 42);
            assert_eq!(
                &contexts[18..],
                serde_json::json!([
                    ["i should let you get back to", ["work", "dinner"]],
                    ["i have to go now, but let's", ["keep", "talk"]],
                    ["thanks for the chat, have a", ["good", "safe"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                &authors[36..],
                serde_json::json!([
                    "i should let you get back to work now, thanks for the chat.",
                    "i should let you get back to dinner, we'll talk another time.",
                    "i have to go now, but let's keep in touch.",
                    "i have to go now, but let's talk again soon.",
                    "thanks for the chat, have a good evening.",
                    "thanks for the chat, have a safe trip home."
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
        [
          "i should let you get back to",
          [
            "work",
            "dinner"
          ]
        ],
        [
          "i have to go now, but let's",
          [
            "keep",
            "talk"
          ]
        ],
        [
          "thanks for the chat, have a",
          [
            "good",
            "safe"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "i should let you get back to work now, thanks for the chat.",
        "i should let you get back to dinner, we'll talk another time.",
        "i have to go now, but let's keep in touch.",
        "i have to go now, but let's talk again soon.",
        "thanks for the chat, have a good evening.",
        "thanks for the chat, have a safe trip home.""#,
            );
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(contexts.len(), 927);
            assert_eq!(
                &contexts[924..],
                serde_json::json!([
                    [
                        "先这样吧",
                        [
                            "先这样吧，今天就聊到这里。",
                            "先这样吧，你早点休息，我们改天再聊。"
                        ]
                    ],
                    [
                        "下次再聊",
                        [
                            "下次再聊，今天和你聊得很开心。",
                            "下次再聊，有空再跟我说说后来的事。"
                        ]
                    ],
                    [
                        "保持联系",
                        [
                            "保持联系，哪天有空再聊聊近况。",
                            "保持联系，有新消息随时告诉我。"
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
      "先这样吧",
      [
        "先这样吧，今天就聊到这里。",
        "先这样吧，你早点休息，我们改天再聊。"
      ]
    ],
    [
      "下次再聊",
      [
        "下次再聊，今天和你聊得很开心。",
        "下次再聊，有空再跟我说说后来的事。"
      ]
    ],
    [
      "保持联系",
      [
        "保持联系，哪天有空再聊聊近况。",
        "保持联系，有新消息随时告诉我。"
      ]
    ]"#,
            );
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            assert_eq!(contexts.len(), 415);
            assert_eq!(
                &contexts[412..],
                serde_json::json!([
                    [
                        "お邪魔しました",
                        [
                            "お邪魔しました。今日はゆっくりお話しできて楽しかったです。",
                            "お邪魔しました。またお会いできるのを楽しみにしています。"
                        ]
                    ],
                    [
                        "おかげ",
                        [
                            "おかげで楽しくお話しできました。今日はありがとうございました。",
                            "おかげで疑問が解けました。またお話しできたらうれしいです。"
                        ]
                    ],
                    [
                        "見送り",
                        [
                            "見送りはここまでで大丈夫です。またゆっくり話しましょう。",
                            "見送りに来てくれてありがとう。また連絡するね。"
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
      "お邪魔しました",
      [
        "お邪魔しました。今日はゆっくりお話しできて楽しかったです。",
        "お邪魔しました。またお会いできるのを楽しみにしています。"
      ]
    ],
    [
      "おかげ",
      [
        "おかげで楽しくお話しできました。今日はありがとうございました。",
        "おかげで疑問が解けました。またお話しできたらうれしいです。"
      ]
    ],
    [
      "見送り",
      [
        "見送りはここまでで大丈夫です。またゆっくり話しましょう。",
        "見送りに来てくれてありがとう。また連絡するね。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire pre-farewell resource changed"
    );
    previous
}

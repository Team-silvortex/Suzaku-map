//! Exact test-only inverse of the finite consideration append.
use ring::digest::{SHA256, digest};
#[path = "farewell_resource.rs"]
mod farewell_resource;

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

pub fn without_consideration(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "acc2f027d7350582293b781956277f3bb960a55446bf53a0fdbd7f74f27d0d8d",
        "zh-Hans" => "4ce6ccbf721e39f37236f40f5ee57eddc6130b07d6415067415e627b42dec39b",
        "ja" => "a476577e9d683001650c4993738f20f0578d0f82ae5ed6c5f8a4450495bd64e1",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let previous = farewell_resource::without_farewell(raw);
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
            assert_eq!(contexts.len(), 18);
            assert_eq!(authors.len(), 36);
            assert_eq!(
                &contexts[15..],
                serde_json::json!([
                    ["thanks for the offer, but i'd rather", ["wait", "stay"]],
                    ["you can take your time and", ["reply", "decide"]],
                    ["please don't feel bad about", ["saying", "asking"]]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                &authors[30..],
                serde_json::json!([
                    "thanks for the offer, but i'd rather wait until later.",
                    "thanks for the offer, but i'd rather stay home tonight.",
                    "you can take your time and reply when you are free.",
                    "you can take your time and decide when you are ready.",
                    "please don't feel bad about saying no if you are busy.",
                    "please don't feel bad about asking for help when you need it."
                ])
                .as_array()
                .unwrap()
            );
            remove_once(
                &mut previous,
                r#",
        [
          "thanks for the offer, but i'd rather",
          [
            "wait",
            "stay"
          ]
        ],
        [
          "you can take your time and",
          [
            "reply",
            "decide"
          ]
        ],
        [
          "please don't feel bad about",
          [
            "saying",
            "asking"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "thanks for the offer, but i'd rather wait until later.",
        "thanks for the offer, but i'd rather stay home tonight.",
        "you can take your time and reply when you are free.",
        "you can take your time and decide when you are ready.",
        "please don't feel bad about saying no if you are busy.",
        "please don't feel bad about asking for help when you need it.""#,
            );
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(contexts.len(), 924);
            assert_eq!(
                &contexts[921..],
                serde_json::json!([
                    [
                        "心意领了",
                        ["心意领了，这次就不麻烦你了。", "心意领了，东西你留着用吧。"]
                    ],
                    [
                        "不用特意",
                        [
                            "不用特意跑一趟，等下次见面再说吧。",
                            "不用特意准备东西，过来坐坐就好。"
                        ]
                    ],
                    [
                        "看你方便",
                        [
                            "看你方便，不着急，晚点回复也可以。",
                            "看你方便，有空的时候再一起商量。"
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
      "心意领了",
      [
        "心意领了，这次就不麻烦你了。",
        "心意领了，东西你留着用吧。"
      ]
    ],
    [
      "不用特意",
      [
        "不用特意跑一趟，等下次见面再说吧。",
        "不用特意准备东西，过来坐坐就好。"
      ]
    ],
    [
      "看你方便",
      [
        "看你方便，不着急，晚点回复也可以。",
        "看你方便，有空的时候再一起商量。"
      ]
    ]"#,
            );
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            assert_eq!(contexts.len(), 412);
            assert_eq!(
                &contexts[409..],
                serde_json::json!([
                    [
                        "ゆっくり",
                        [
                            "ゆっくりで大丈夫です。急がなくても構いません。",
                            "ゆっくり考えてから、返事をもらえれば大丈夫です。"
                        ]
                    ],
                    [
                        "よろしければ",
                        [
                            "よろしければ、都合のよいときに返事をください。",
                            "よろしければ、無理のない範囲で手伝ってください。"
                        ]
                    ],
                    [
                        "ご親切",
                        [
                            "ご親切にありがとうございます。今回は自分でやってみます。",
                            "ご親切なお申し出ですが、今日は大丈夫です。"
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
      "ゆっくり",
      [
        "ゆっくりで大丈夫です。急がなくても構いません。",
        "ゆっくり考えてから、返事をもらえれば大丈夫です。"
      ]
    ],
    [
      "よろしければ",
      [
        "よろしければ、都合のよいときに返事をください。",
        "よろしければ、無理のない範囲で手伝ってください。"
      ]
    ],
    [
      "ご親切",
      [
        "ご親切にありがとうございます。今回は自分でやってみます。",
        "ご親切なお申し出ですが、今日は大丈夫です。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire pre-consideration resource changed"
    );
    previous
}

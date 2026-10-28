//! Exact test-only inverse of the daily services and household append.
use ring::digest::{SHA256, digest};

#[path = "daily_support_resource.rs"]
mod daily_support_resource;

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

pub fn without_daily_services(raw: &str) -> String {
    let raw = daily_support_resource::without_daily_support(raw);
    let raw = raw.as_str();
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "35c06dc2388a1fa2128c728adbd6a751d935176279d18223abfa6a7d698f049b",
        "zh-Hans" => "2d05d1c791a53d96f3ad62641ad4a2ccf067c3f522765637bac0d189968443f2",
        "ja" => "28f3a13d4d9247b0b15968972b9c363f50c37d6c51b1b8efdca288fe23ae3c4b",
        _ => panic!("unexpected resource language"),
    };
    let old = match language {
        "en" => {
            let layer = value["word_layers"].as_array().unwrap().last().unwrap();
            let contexts = layer["next_words"].as_array().unwrap().len();
            let sentences = layer["sentences"].as_array().unwrap().len();
            if contexts <= 40 && sentences <= 80 {
                true
            } else {
                assert_eq!(
                    (contexts, sentences),
                    (43, 86),
                    "unexpected daily-services shape"
                );
                false
            }
        }
        "zh-Hans" | "ja" => {
            let contexts = value["continuations"].as_array().unwrap().len();
            let (old, new) = if language == "ja" {
                (434, 437)
            } else {
                (946, 949)
            };
            if contexts <= old {
                true
            } else {
                assert_eq!(contexts, new, "unexpected daily-services shape");
                false
            }
        }
        _ => unreachable!(),
    };
    if old {
        return raw.to_owned();
    }
    let mut previous = raw.to_owned();
    match language {
        "en" => {
            remove_once(
                &mut previous,
                r#",
        [
          "before we leave, could you check the",
          [
            "front",
            "back"
          ]
        ],
        [
          "please remember to bring your",
          [
            "keys",
            "phone"
          ]
        ],
        [
          "are you having dinner",
          [
            "at",
            "with"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "before we leave, could you check the front door is locked?",
        "before we leave, could you check the back window is closed?",
        "please remember to bring your keys when you go out.",
        "please remember to bring your phone in case we need to reach you.",
        "are you having dinner at home tonight?",
        "are you having dinner with us tonight?""#,
            );
        }
        "zh-Hans" => remove_once(
            &mut previous,
            r#",
    [
      "配送",
      [
        "配送时间可以改到明天下午吗？今天家里没人。",
        "配送前麻烦给我打个电话，我好下楼取件。"
      ]
    ],
    [
      "服务台",
      [
        "服务台在二楼电梯旁，需要帮助可以去那里问一下。",
        "服务台可以帮忙查询遗失物品吗？我刚才找不到钥匙了。"
      ]
    ],
    [
      "柜台",
      [
        "柜台已经关了，请问明天几点可以办理？",
        "柜台工作人员让我带上身份证，明天再来一趟。"
      ]
    ]"#,
        ),
        "ja" => remove_once(
            &mut previous,
            r#",
    [
      "予約",
      [
        "予約の時間を明日の午後に変更できますか。",
        "予約した名前を伝えますので、確認をお願いします。"
      ]
    ],
    [
      "配送",
      [
        "配送は明日の午後に変更できますか。今日は家にいません。",
        "配送前に電話をいただけると助かります。"
      ]
    ],
    [
      "受付",
      [
        "受付で番号を受け取ってから、こちらでお待ちください。",
        "受付は何時まで開いていますか。必要な持ち物も教えてください。"
      ]
    ]"#,
        ),
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire pre-daily-services resource changed"
    );
    previous
}

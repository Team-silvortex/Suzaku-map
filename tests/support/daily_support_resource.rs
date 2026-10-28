//! Exact test-only inverse of the daily support append.
use ring::digest::{SHA256, digest};

#[path = "daily_errand_followup_resource.rs"]
mod daily_errand_followup_resource;

fn remove_once(raw: &mut String, block: &str) {
    assert_eq!(
        raw.matches(block).count(),
        1,
        "exact appended bytes changed"
    );
    let start = raw.find(block).unwrap();
    raw.replace_range(start..start + block.len(), "");
}

pub fn without_daily_support(raw: &str) -> String {
    let raw = daily_errand_followup_resource::without_daily_errand_followup(raw);
    let raw = raw.as_str();
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let (old, expected) = match language {
        "en" => {
            let layer = value["word_layers"].as_array().unwrap().last().unwrap();
            let shape = (
                layer["next_words"].as_array().unwrap().len(),
                layer["sentences"].as_array().unwrap().len(),
            );
            let old = if shape.0 <= 43 && shape.1 <= 86 {
                true
            } else {
                assert_eq!(shape, (46, 92), "unexpected daily-support shape");
                false
            };
            (
                old,
                "b9bb2e0661240123a7da5d2d58433d21c86c5c25d80905ed82cdf5a449afae30",
            )
        }
        "zh-Hans" | "ja" => {
            let count = value["continuations"].as_array().unwrap().len();
            let (old_count, new_count, hash) = if language == "ja" {
                (
                    437,
                    440,
                    "90cc09724b45e9b2234f99a249309548d0c1a3403f3ba2c2e6a8241dc8728162",
                )
            } else {
                (
                    949,
                    952,
                    "468fac0c8fdff746680dc46ac2b477307324e5adff7dfebe6332f3f970f5eeb4",
                )
            };
            let old = if count <= old_count {
                true
            } else {
                assert_eq!(count, new_count, "unexpected daily-support shape");
                false
            };
            (old, hash)
        }
        _ => panic!("unexpected resource language"),
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
          "before we order, could you check whether the",
          [
            "soup",
            "sauce"
          ]
        ],
        [
          "could we take a break and get some",
          [
            "fresh",
            "water"
          ]
        ],
        [
          "please send me a message when you reach the",
          [
            "station",
            "hotel"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "before we order, could you check whether the soup has milk?",
        "before we order, could you check whether the sauce has nuts?",
        "could we take a break and get some fresh air?",
        "could we take a break and get some water?",
        "please send me a message when you reach the station.",
        "please send me a message when you reach the hotel.""#,
            );
        }
        "zh-Hans" => remove_once(
            &mut previous,
            r#",
    [
      "取件凭证",
      [
        "取件凭证我已经保存，等会儿去柜台出示。",
        "取件凭证没有收到，可以麻烦你再发一次吗？"
      ]
    ],
    [
      "预约凭证",
      [
        "预约凭证我已经保存好了，到现场时可以出示。",
        "预约凭证找不到了，可以帮我重新发送一份吗？"
      ]
    ],
    [
      "报修记录",
      [
        "报修记录里写了设备的问题，麻烦先看一下。",
        "报修记录可以在哪里查看？我想确认上门时间。"
      ]
    ]"#,
        ),
        "ja" => remove_once(
            &mut previous,
            r#",
    [
      "紛失届",
      [
        "紛失届を出したいのですが、どこで手続きできますか。",
        "紛失届に必要な情報を確認してから、窓口へ行きます。"
      ]
    ],
    [
      "受付時間",
      [
        "受付時間は何時から何時までですか。",
        "受付時間に間に合わない場合は、明日伺います。"
      ]
    ],
    [
      "不具合",
      [
        "不具合が出たときの状況を、写真と一緒に送ります。",
        "不具合について相談したいので、連絡先を教えてください。"
      ]
    ]"#,
        ),
        _ => unreachable!(),
    }
    let actual: String = digest(&SHA256, previous.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        actual, expected,
        "entire pre-daily-support resource changed"
    );
    previous
}

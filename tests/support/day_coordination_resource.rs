//! Exact test-only inverse of the daily coordination append.
use ring::digest::{SHA256, digest};

fn remove_once(raw: &mut String, block: &str) {
    assert_eq!(
        raw.matches(block).count(),
        1,
        "exact appended bytes changed"
    );
    let start = raw.find(block).unwrap();
    raw.replace_range(start..start + block.len(), "");
}

pub fn without_day_coordination(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let (old, expected) = match language {
        "en" => {
            let layer = value["word_layers"].as_array().unwrap().last().unwrap();
            let shape = (
                layer["next_words"].as_array().unwrap().len(),
                layer["sentences"].as_array().unwrap().len(),
            );
            let old = if shape.0 <= 49 && shape.1 <= 98 {
                true
            } else {
                assert_eq!(shape, (53, 106), "unexpected daily-coordination shape");
                false
            };
            (
                old,
                "8cc9c27d3a57365465509e074a321af4644e8efbd52e365e9c78b65b85417c75",
            )
        }
        "zh-Hans" | "ja" => {
            let count = value["continuations"].as_array().unwrap().len();
            let (old_count, new_count, hash) = if language == "ja" {
                (
                    443,
                    447,
                    "2375285078cafd1d90114380da265a6a477a3e8c912379bbe345fa4e59dd8a4c",
                )
            } else {
                (
                    955,
                    959,
                    "098c8388d0f21481d35cab328f940ebaafdf2a208b15f192c8c1e07133393e58",
                )
            };
            let old = if count <= old_count {
                true
            } else {
                assert_eq!(count, new_count, "unexpected daily-coordination shape");
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
          "i'm free for a quick call",
          [
            "after",
            "before"
          ]
        ],
        [
          "let's leave a little extra time for",
          [
            "questions",
            "travel"
          ]
        ],
        [
          "if the link does not work, please",
          [
            "send",
            "tell"
          ]
        ],
        [
          "would you prefer to meet",
          [
            "before",
            "after"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "i'm free for a quick call after lunch if you have time.",
        "i'm free for a quick call before the meeting starts.",
        "let's leave a little extra time for questions at the end.",
        "let's leave a little extra time for travel in case there is traffic.",
        "if the link does not work, please send me another one.",
        "if the link does not work, please tell me what you see on the screen.",
        "would you prefer to meet before lunch tomorrow?",
        "would you prefer to meet after work on friday?""#,
            );
        }
        "zh-Hans" => remove_once(
            &mut previous,
            r#",
    [
      "日程",
      [
        "日程我已经记下了，如果时间有变化，请提前告诉我。",
        "日程里还有一处时间冲突，我们先确认哪一项需要调整。"
      ]
    ],
    [
      "集合点",
      [
        "集合点改到车站东门，到了以后在群里说一声。",
        "集合点附近有一家便利店，我们可以在那里等。"
      ]
    ],
    [
      "预计到达",
      [
        "预计到达时间是下午两点，路上如果堵车我会及时告诉你。",
        "预计到达时间还不确定，我出发后再发消息确认。"
      ]
    ],
    [
      "提醒一下",
      [
        "提醒一下，明天出门前记得带上证件和预约凭证。",
        "提醒一下，这份材料需要在今天下班前发给对方。"
      ]
    ]"#,
        ),
        "ja" => remove_once(
            &mut previous,
            r#",
    [
      "返信待ち",
      [
        "返信待ちのため、予定が決まり次第お知らせします。",
        "返信待ちの間に、先に必要な資料をそろえておきます。"
      ]
    ],
    [
      "予約確認",
      [
        "予約確認のメールが届いているか、出発前に見ておきます。",
        "予約確認をお願いしたいのですが、名前をお伝えすればよいですか。"
      ]
    ],
    [
      "合流地点",
      [
        "合流地点は駅の東口にしましょう。着いたら連絡してください。",
        "合流地点を変える場合は、先に地図の位置を送ってください。"
      ]
    ],
    [
      "延長",
      [
        "延長をお願いしたいのですが、いつまで借りられますか。",
        "延長が難しい場合は、予定どおり明日返却します。"
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
        "entire pre-daily-coordination resource changed"
    );
    previous
}

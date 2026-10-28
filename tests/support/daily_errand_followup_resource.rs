//! Exact test-only inverse of the daily errand follow-up append.
use ring::digest::{SHA256, digest};

#[path = "day_coordination_resource.rs"]
mod day_coordination_resource;

fn remove_once(raw: &mut String, block: &str) {
    assert_eq!(
        raw.matches(block).count(),
        1,
        "exact appended bytes changed"
    );
    let start = raw.find(block).unwrap();
    raw.replace_range(start..start + block.len(), "");
}

pub fn without_daily_errand_followup(raw: &str) -> String {
    let raw = day_coordination_resource::without_day_coordination(raw);
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
            let old = if shape.0 <= 46 && shape.1 <= 92 {
                true
            } else {
                assert_eq!(shape, (49, 98), "unexpected daily-errand shape");
                false
            };
            (
                old,
                "afd080dbdb43bbf996f91688af5b9ee2c7b95648d7885f1c0213e234b8b96303",
            )
        }
        "zh-Hans" | "ja" => {
            let count = value["continuations"].as_array().unwrap().len();
            let (old_count, new_count, hash) = if language == "ja" {
                (
                    440,
                    443,
                    "a77343ca09969c8709bb9500508f68ae1517d6c7a7431c29a8bf1e3aaa9252cb",
                )
            } else {
                (
                    952,
                    955,
                    "650322cc4f160f368f09ed7c917a2fbaab2c9b14b11e8fc39e198ace72a2f58e",
                )
            };
            let old = if count <= old_count {
                true
            } else {
                assert_eq!(count, new_count, "unexpected daily-errand shape");
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
          "when the weather is clear, we can",
          [
            "walk",
            "leave"
          ]
        ],
        [
          "please put the receipt in the",
          [
            "bag",
            "drawer"
          ]
        ],
        [
          "i'll charge my phone before",
          [
            "leaving",
            "dinner"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "when the weather is clear, we can walk to the station.",
        "when the weather is clear, we can leave for the park.",
        "please put the receipt in the bag with the items.",
        "please put the receipt in the drawer with the keys.",
        "i'll charge my phone before leaving for the station.",
        "i'll charge my phone before dinner so it is ready later.""#,
            );
        }
        "zh-Hans" => remove_once(
            &mut previous,
            r#",
    [
      "申请表",
      [
        "申请表填好后请先检查联系方式，再提交给工作人员。",
        "申请表需要附上哪些材料？我想提前准备好。"
      ]
    ],
    [
      "备用电池",
      [
        "备用电池放在抽屉里，出门前记得带上。",
        "备用电池还有电吗？我先检查一下再装进去。"
      ]
    ],
    [
      "停车场",
      [
        "停车场的入口在大楼后面，沿着指示牌走就能看到。",
        "停车场的缴费方式是什么？我想先确认一下。"
      ]
    ]"#,
        ),
        "ja" => remove_once(
            &mut previous,
            r#",
    [
      "申請書",
      [
        "申請書を書き終えたら、連絡先を確認して提出します。",
        "申請書と一緒に必要な書類を教えていただけますか。"
      ]
    ],
    [
      "送り状",
      [
        "送り状に住所を書いたら、発送前にもう一度確認します。",
        "送り状の番号を教えていただけますか。配送状況を確認します。"
      ]
    ],
    [
      "順序",
      [
        "順序を確認してから、一つずつ作業を進めます。",
        "順序が変わる場合は、先に知らせていただけると助かります。"
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
    assert_eq!(actual, expected, "entire pre-daily-errand resource changed");
    previous
}

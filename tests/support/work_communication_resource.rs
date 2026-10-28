//! Exact test-only inverse of the finite work_communication append.
use ring::digest::{SHA256, digest};
#[path = "work_clarification_resource.rs"]
mod work_clarification_resource;

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

pub fn without_work_communication(raw: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let expected = match language {
        "en" => "9c6538551a3b80881b1f5db8a4c4d137ff0da33cbab1dfc89f0b682a974b2f63",
        "zh-Hans" => "036f64fa73f0b3bca8b5a784c03263642c4bc02c371e62b3f452764fd3a1ad7b",
        "ja" => "a254d6512fe44f81b1bed3c78c17a64583ad2a77d987205b8086cd90c05236f6",
        _ => panic!("unexpected resource language"),
    };
    if sha(raw) == expected {
        return raw.to_owned();
    }
    let previous = work_clarification_resource::without_work_clarification(raw);
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
            assert_eq!(contexts.len(), 31);
            assert_eq!(authors.len(), 62);
            assert_eq!(
                &contexts[27..],
                serde_json::json!([
                    [
                        "we're on track, but we still need",
                        ["approval", "feedback"]
                    ],
                    [
                        "before we begin, can we agree on the",
                        ["scope", "deadline"]
                    ],
                    ["for the next team meeting, should we", ["invite", "share"]],
                    [
                        "once you have reviewed the handover, could you",
                        ["confirm", "flag"]
                    ]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(&authors[54..], serde_json::json!(["we're on track, but we still need approval before we can continue.", "we're on track, but we still need feedback on the latest draft.", "before we begin, can we agree on the scope so everyone knows what to expect?", "before we begin, can we agree on the deadline and what needs to be ready?", "for the next team meeting, should we invite everyone who needs to be there?", "for the next team meeting, should we share the draft before we talk?", "once you have reviewed the handover, could you confirm that everything is clear?", "once you have reviewed the handover, could you flag anything that is missing?"]).as_array().unwrap());
            remove_once(
                &mut previous,
                r#",
        [
          "we're on track, but we still need",
          [
            "approval",
            "feedback"
          ]
        ],
        [
          "before we begin, can we agree on the",
          [
            "scope",
            "deadline"
          ]
        ],
        [
          "for the next team meeting, should we",
          [
            "invite",
            "share"
          ]
        ],
        [
          "once you have reviewed the handover, could you",
          [
            "confirm",
            "flag"
          ]
        ]"#,
            );
            remove_once(
                &mut previous,
                r#",
        "we're on track, but we still need approval before we can continue.",
        "we're on track, but we still need feedback on the latest draft.",
        "before we begin, can we agree on the scope so everyone knows what to expect?",
        "before we begin, can we agree on the deadline and what needs to be ready?",
        "for the next team meeting, should we invite everyone who needs to be there?",
        "for the next team meeting, should we share the draft before we talk?",
        "once you have reviewed the handover, could you confirm that everything is clear?",
        "once you have reviewed the handover, could you flag anything that is missing?""#,
            );
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            assert_eq!(contexts.len(), 937);
            assert_eq!(
                &contexts[933..],
                serde_json::json!([
                    [
                        "同步进度",
                        [
                            "同步进度：已经完成的部分我发给你，剩下的明天下午补上。",
                            "同步进度时，也请说一下卡在哪里，我们一起想办法。"
                        ]
                    ],
                    [
                        "需求",
                        [
                            "需求我先确认一下：这次只改文字，不调整页面，对吗？",
                            "需求有变化的话，请把新增和删掉的部分分开标出来。"
                        ]
                    ],
                    [
                        "线上会议",
                        [
                            "线上会议安排在明天下午三点，你看方便吗？",
                            "线上会议开始前，麻烦先把要讨论的问题发到群里。"
                        ]
                    ],
                    [
                        "反馈汇总",
                        [
                            "反馈汇总已经整理好了，交接前请看一下还没解决的问题。",
                            "反馈汇总里我标出了需要确认的地方，麻烦看完后告诉我。"
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
      "同步进度",
      [
        "同步进度：已经完成的部分我发给你，剩下的明天下午补上。",
        "同步进度时，也请说一下卡在哪里，我们一起想办法。"
      ]
    ],
    [
      "需求",
      [
        "需求我先确认一下：这次只改文字，不调整页面，对吗？",
        "需求有变化的话，请把新增和删掉的部分分开标出来。"
      ]
    ],
    [
      "线上会议",
      [
        "线上会议安排在明天下午三点，你看方便吗？",
        "线上会议开始前，麻烦先把要讨论的问题发到群里。"
      ]
    ],
    [
      "反馈汇总",
      [
        "反馈汇总已经整理好了，交接前请看一下还没解决的问题。",
        "反馈汇总里我标出了需要确认的地方，麻烦看完后告诉我。"
      ]
    ]"#,
            );
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            assert_eq!(contexts.len(), 425);
            assert_eq!(
                &contexts[421..],
                serde_json::json!([
                    [
                        "担当",
                        [
                            "担当している部分は、明日の午前中に仕上げる予定です。",
                            "担当を引き継ぐ前に、今どこまで進んだかまとめます。"
                        ]
                    ],
                    [
                        "対象",
                        [
                            "対象になる作業と、今回は含めない作業を先に確認しましょう。",
                            "対象はこの三つで合っていますか。必要な条件も教えてください。"
                        ]
                    ],
                    [
                        "会議",
                        [
                            "会議は明日の午後三時からで、ご都合はいかがですか。",
                            "会議の前に、話し合いたいことを二つだけ送ってもらえますか。"
                        ]
                    ],
                    [
                        "変更履歴",
                        [
                            "変更履歴を見ながら、直した箇所を一緒に確認しましょう。",
                            "変更履歴に理由も残しておくので、引き継ぎのときに確認してください。"
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
      "担当",
      [
        "担当している部分は、明日の午前中に仕上げる予定です。",
        "担当を引き継ぐ前に、今どこまで進んだかまとめます。"
      ]
    ],
    [
      "対象",
      [
        "対象になる作業と、今回は含めない作業を先に確認しましょう。",
        "対象はこの三つで合っていますか。必要な条件も教えてください。"
      ]
    ],
    [
      "会議",
      [
        "会議は明日の午後三時からで、ご都合はいかがですか。",
        "会議の前に、話し合いたいことを二つだけ送ってもらえますか。"
      ]
    ],
    [
      "変更履歴",
      [
        "変更履歴を見ながら、直した箇所を一緒に確認しましょう。",
        "変更履歴に理由も残しておくので、引き継ぎのときに確認してください。"
      ]
    ]"#,
            );
        }
        _ => unreachable!(),
    }
    assert_eq!(
        sha(&previous),
        expected,
        "entire pre-work_communication resource changed"
    );
    previous
}

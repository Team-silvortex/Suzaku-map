//! Exact test-only inverse of the presence dialogue append, never candidate projection.
use ring::digest::{SHA256, digest};
#[path = "turntaking_resource.rs"]
mod turntaking_resource;
pub use turntaking_resource::without_turntaking;

pub fn without_presence(raw: &str) -> String {
    let previous = without_turntaking(raw);
    let raw = previous.as_str();
    let value: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = value["language"].as_str().unwrap();
    let (first, after, expected) = match language {
        "en" => {
            let layers = value["word_layers"].as_array().unwrap();
            if layers.len() > 63 {
                assert_eq!(layers.len(), 64);
                assert_eq!(
                    layers[63],
                    serde_json::json!({
                        "id": "daily_presence_dialogue", "words": [],
                        "next_words": [
                            ["i need to step away from the", ["chat", "screen"]],
                            ["i'm back and can", ["continue", "listen"]],
                            ["let's continue this after", ["lunch", "dinner"]]
                        ],
                        "sentences": [
                            "i need to step away from the chat for a few minutes.",
                            "i need to step away from the screen until after lunch.",
                            "i'm back and can continue where we left off.",
                            "i'm back and can listen to the rest now.",
                            "let's continue this after lunch when we're both free.",
                            "let's continue this after dinner if you have time."
                        ]
                    })
                );
            } else {
                assert_eq!(layers.len(), 63);
            }
            (
                ",\n    {\n      \"id\": \"daily_presence_dialogue\",",
                "\n  ],\n  \"phrase_endings\": [",
                "899d6e92d29729e7ea31d5d3e4cda420f9c712381caf4dded030a9a99e4a7bd3",
            )
        }
        "zh-Hans" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 3442);
            if contexts.len() > 906 {
                assert_eq!(contexts.len(), 909);
                assert_eq!(
                    &contexts[906..],
                    serde_json::json!([
                        [
                            "暂停一下",
                            [
                                "暂停一下，我去拿点东西，马上回来。",
                                "暂停一下，等我回来再接着说。"
                            ]
                        ],
                        [
                            "几点回来",
                            [
                                "几点回来呢？我想等你一起吃饭。",
                                "几点回来方便？我们到时候再继续聊。"
                            ]
                        ],
                        [
                            "稍后讨论",
                            [
                                "稍后讨论也可以，我先把手头的事做完。",
                                "稍后讨论这个问题，等大家都有空再继续。"
                            ]
                        ]
                    ])
                    .as_array()
                    .unwrap()
                );
            } else {
                assert_eq!(contexts.len(), 906);
            }
            (
                ",\n    [\n      \"暂停一下\",",
                "\n  ]\n}",
                "61734b2ea54c008bff68cd7ae7afb528ab57b29de493e555f63063c145ea6164",
            )
        }
        "ja" => {
            let contexts = value["continuations"].as_array().unwrap();
            assert_eq!(value["readings"].as_array().unwrap().len(), 638);
            if contexts.len() > 394 {
                assert_eq!(contexts.len(), 397);
                assert_eq!(
                    &contexts[394..],
                    serde_json::json!([
                        [
                            "ただいま",
                            [
                                "ただいま。さっきの続きを一緒に確認してもいいですか。",
                                "ただいま戻りました。途中で変わった点があれば教えてください。"
                            ]
                        ],
                        [
                            "不在",
                            [
                                "不在の間の連絡は、戻ってからまとめて確認します。",
                                "不在になる時間だけ、先に伝えておきます。"
                            ]
                        ],
                        [
                            "退室",
                            [
                                "退室する前に、戻る予定の時間だけお伝えします。",
                                "退室しますが、戻ったらさっきの続きを一緒に確認しましょう。"
                            ]
                        ]
                    ])
                    .as_array()
                    .unwrap()
                );
            } else {
                assert_eq!(contexts.len(), 394);
            }
            (
                ",\n    [\n      \"ただいま\",",
                "\n  ]\n}",
                "68019e118f4422174fe6f6ba063129dba1ba70f87dd9e00a9c16e0f25b7d5269",
            )
        }
        _ => panic!("unexpected resource language"),
    };
    let mut previous = raw.to_owned();
    if raw.contains(first) {
        assert_eq!(raw.matches(first).count(), 1);
        assert_eq!(raw.matches(after).count(), 1);
        let (from, to) = (raw.find(first).unwrap(), raw.find(after).unwrap());
        assert!(from < to);
        previous.replace_range(from..to, "");
    }
    let hash = digest(&SHA256, previous.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(hash, expected, "entire published resource prefix changed");
    previous
}

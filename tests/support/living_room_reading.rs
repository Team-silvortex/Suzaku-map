//! Test-only inverse of the single living-room reading, never a runtime hook.
use ring::digest::{SHA256, digest};

const READINGS_END: &str = "\n  ],\n  \"continuations\": [";
const APPEND: &str =
    ",\n    {\"reading\": \"yao'kong'qi\", \"text\": \"遥控器\", \"kind\": \"word\"}";
const OLD_PREFIX_BYTES: usize = 244343;
const OLD_PREFIX_SHA256: &str = "307358b0e40ad39b99af2f8d610cc08640bfa515afce48cd6fe9b75315d459a3";

/// Remove only the reviewed new tail reading before an older raw-resource guard.
///
/// Accept the exact old 3441-reading / 888-context state for test-first, or the
/// new 3442-reading state with the exact three living-room records at 888..891;
/// later contexts may only follow that fixed window. The second state must have
/// precisely the new compact three-field tuple at the raw readings tail. Both states bind the
/// complete old readings byte prefix, including flags, order and JSON spelling.
/// Context bytes are never projected: the calling suite keeps its old inverse
/// and complete old-resource hash. No historic hash is replaced by this helper.
pub fn without_living_room_reading(raw: &str) -> String {
    let data: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = data["language"].as_str().unwrap();
    if language != "zh-Hans" {
        assert!(matches!(language, "en" | "ja"));
        return raw.to_owned();
    }

    let readings = data["readings"].as_array().unwrap();
    let contexts = data["continuations"].as_array().unwrap();
    assert_eq!(raw.matches(READINGS_END).count(), 1);
    let end = raw.find(READINGS_END).unwrap();
    let prefix = &raw[..end];
    let old_prefix = match readings.len() {
        3441 => {
            // A missing post-append reading is not a valid before-data state.
            assert_eq!(contexts.len(), 888);
            assert!(!prefix.contains(APPEND));
            prefix
        }
        3442 => {
            assert!(contexts.len() >= 891);
            assert_eq!(
                &contexts[888..891],
                serde_json::json!([
                    [
                        "遥控器",
                        [
                            "遥控器放在沙发旁边的小桌上。",
                            "遥控器在纸巾盒后面，我刚才看见了。"
                        ]
                    ],
                    [
                        "纸巾盒",
                        [
                            "纸巾盒放在茶几的右边。",
                            "纸巾盒在电视柜旁边，我帮你拿过来。"
                        ]
                    ],
                    [
                        "眼镜盒",
                        ["眼镜盒放在靠窗的小桌上。", "眼镜盒在抽屉的前面，你看看。"]
                    ]
                ])
                .as_array()
                .unwrap()
            );
            assert_eq!(
                readings[3441],
                serde_json::json!({
                    "reading": "yao'kong'qi",
                    "text": "遥控器",
                    "kind": "word"
                })
            );
            assert_eq!(
                readings
                    .iter()
                    .filter(|entry| entry["reading"] == "yao'kong'qi" || entry["text"] == "遥控器")
                    .count(),
                1
            );
            assert_eq!(prefix.matches(APPEND).count(), 1);
            prefix
                .strip_suffix(APPEND)
                .expect("living-room reading is not the exact raw tail")
        }
        count => panic!("unexpected readings state: {count}"),
    };
    assert_eq!(old_prefix.len(), OLD_PREFIX_BYTES);
    let hash = digest(&SHA256, old_prefix.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(hash, OLD_PREFIX_SHA256);

    let mut previous = String::with_capacity(old_prefix.len() + raw.len() - end);
    previous.push_str(old_prefix);
    previous.push_str(&raw[end..]);
    previous
}

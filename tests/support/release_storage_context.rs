//! Test-only inverse of one withdrawn storage context; never a runtime hook.
//! Future context tails are allowed, but the exact reviewed slot and complete
//! readings byte prefix stay fixed. Calling legacy raw hashes remain authoritative.
use ring::digest::{SHA256, digest};

const READINGS_END: &str = "\n  ],\n  \"continuations\": [";
const PREFIX_BYTES: usize = 244412;
const PREFIX_SHA: &str = "0cc6d154280141eefaba4402a46f9380ab6dfa403788b4f25e95f748043f87c4";
const NEW: &str = "    [\n      \"耳塞\",\n      [\n        \"耳塞放在随身的小袋里。\",\n        \"耳塞在抽屉前面，我帮你拿过来。\"\n      ]\n    ]";
const OLD: &str = "    [\n      \"收纳袋\",\n      [\n        \"收纳袋放在衣柜下面那层。\",\n        \"收纳袋里还放着帽子和围巾。\"\n      ]\n    ]";

/// Restore only the exact three withdrawn strings before a legacy raw guard.
/// This does not alter candidates, readings, scores, production data or old hashes.
pub fn restore_pre_release_storage_context(raw: &str) -> String {
    let data: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = data["language"].as_str().unwrap();
    if language != "zh-Hans" {
        assert!(matches!(language, "en" | "ja"));
        return raw.to_owned();
    }
    let readings = data["readings"].as_array().unwrap();
    let contexts = data["continuations"].as_array().unwrap();
    assert_eq!(readings.len(), 3442);
    assert!(contexts.len() >= 906);
    assert_eq!(
        contexts[896],
        serde_json::json!([
            "耳塞",
            ["耳塞放在随身的小袋里。", "耳塞在抽屉前面，我帮你拿过来。"]
        ])
    );
    assert_eq!(contexts.iter().filter(|row| row[0] == "耳塞").count(), 1);
    assert!(contexts.iter().all(|row| row[0] != "收纳袋"));
    for (reading, word) in [("er'sai", "耳塞"), ("shou'na'dai", "收纳袋")] {
        let matches = readings
            .iter()
            .filter(|entry| entry["text"] == word)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0],
            &serde_json::json!({"reading": reading, "text": word, "kind": "word"})
        );
    }
    assert_eq!(raw.matches(READINGS_END).count(), 1);
    let prefix = &raw[..raw.find(READINGS_END).unwrap()];
    assert_eq!(prefix.len(), PREFIX_BYTES);
    let hash = digest(&SHA256, prefix.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(hash, PREFIX_SHA);
    assert_eq!(raw.matches(NEW).count(), 1);
    assert_eq!(raw.matches(OLD).count(), 0);
    raw.replacen(NEW, OLD, 1)
}

const NEW_PROGRESS: &str = "    [\n      \"未完成\",\n      [\n        \"未完成的部分我先列出来，等信息齐了再继续。\",\n        \"未完成的地方还需要确认，今天先到这里吧。\"\n      ]\n    ]";
const OLD_PROGRESS: &str = "    [\n      \"卡住\",\n      [\n        \"卡住的地方我先记下来，等你有空再一起看。\",\n        \"卡住了也没关系，我们先确认缺的是哪条信息。\"\n      ]\n    ]";

/// Independently restore the second withdrawn context at its reviewed slot.
/// No entire context-tail hash is locked here; callers retain their old raw hash.
pub fn restore_pre_release_progress_context(raw: &str) -> String {
    let data: serde_json::Value = serde_json::from_str(raw).unwrap();
    let language = data["language"].as_str().unwrap();
    if language != "zh-Hans" {
        assert!(matches!(language, "en" | "ja"));
        return raw.to_owned();
    }
    let readings = data["readings"].as_array().unwrap();
    let contexts = data["continuations"].as_array().unwrap();
    assert_eq!(readings.len(), 3442);
    assert!(contexts.len() >= 906);
    assert_eq!(
        contexts[903],
        serde_json::json!([
            "未完成",
            [
                "未完成的部分我先列出来，等信息齐了再继续。",
                "未完成的地方还需要确认，今天先到这里吧。"
            ]
        ])
    );
    assert_eq!(contexts.iter().filter(|row| row[0] == "未完成").count(), 1);
    assert!(contexts.iter().all(|row| row[0] != "卡住"));
    for (reading, word) in [("wei'wan'cheng", "未完成"), ("ka'zhu", "卡住")] {
        let matches = readings
            .iter()
            .filter(|entry| entry["text"] == word)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0],
            &serde_json::json!({"reading": reading, "text": word, "kind": "word"})
        );
    }
    assert_eq!(raw.matches(READINGS_END).count(), 1);
    let prefix = &raw[..raw.find(READINGS_END).unwrap()];
    assert_eq!(prefix.len(), PREFIX_BYTES);
    let hash = digest(&SHA256, prefix.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(hash, PREFIX_SHA);
    assert_eq!(raw.matches(NEW_PROGRESS).count(), 1);
    assert_eq!(raw.matches(OLD_PROGRESS).count(), 0);
    raw.replacen(NEW_PROGRESS, OLD_PROGRESS, 1)
}

/// Restore both six-string withdrawals before the unchanged pre-release chain.
pub fn restore_pre_release_contexts(raw: &str) -> String {
    restore_pre_release_storage_context(&restore_pre_release_progress_context(raw))
}

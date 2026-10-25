//! Release-only context withdrawals; original legacy oracles stay unchanged.
#[path = "support/presence_resource.rs"]
mod presence_resource;
#[path = "support/release_storage_context.rs"]
mod release_storage_context;

use release_storage_context::{
    restore_pre_release_contexts as restore, restore_pre_release_progress_context as progress_only,
    restore_pre_release_storage_context as storage_only,
};
use ring::digest::{SHA256, digest};
use std::collections::HashSet;
use suzaku_map::ime::{
    Candidate, CommitOptions, EngineConfig, XRTabletImeEngine,
    candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
};

const ZH: &str = include_str!("../data/lexicons/zh-Hans.json");
const BEFORE_SHA: &str = "757614e5835d74559c81ea75feb2ab7841052bfb4761b83a6dd6b6ab27b09f15";
// Final source SHA independently supplied by the root agent after both withdrawals.
const CURRENT_SHA: &str = "61734b2ea54c008bff68cd7ae7afb528ab57b29de493e555f63063c145ea6164";
const AUTHORS: [&str; 2] = ["耳塞放在随身的小袋里。", "耳塞在抽屉前面，我帮你拿过来。"];
const PROGRESS_AUTHORS: [&str; 2] = [
    "未完成的部分我先列出来，等信息齐了再继续。",
    "未完成的地方还需要确认，今天先到这里吧。",
];

fn sha(raw: &str) -> String {
    digest(&SHA256, raw.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn pool_hash(candidates: &[Candidate]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for candidate in candidates {
        for byte in candidate
            .text
            .bytes()
            .chain([0])
            .chain(candidate.label.bytes())
            .chain([0])
            .chain(candidate.kind.id().bytes())
            .chain([0])
            .chain(candidate.source.id().bytes())
            .chain([0])
            .chain(candidate.score.to_bits().to_le_bytes())
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    hash
}

// Re-encode only the context array, retaining the exact readings-prefix bytes.
fn with_contexts(raw: &str, contexts: &serde_json::Value) -> String {
    let marker = "  \"continuations\": ";
    assert_eq!(raw.matches(marker).count(), 1);
    let start = raw.find(marker).unwrap() + marker.len();
    let rendered = serde_json::to_string_pretty(contexts)
        .unwrap()
        .replace('\n', "\n  ");
    format!("{}{}\n}}\n", &raw[..start], rendered)
}

#[test]
fn release_storage_exact_six_string_inverse_preserves_readings_and_all_other_contexts() {
    // Preserve this release's exact 906-context snapshot, not runtime candidates.
    let published = presence_resource::without_presence(ZH);
    let zh = published.as_str();
    assert_eq!(sha(zh), CURRENT_SHA);
    let previous = restore(zh);
    assert_eq!(sha(&previous), BEFORE_SHA);
    let old: serde_json::Value = serde_json::from_str(&previous).unwrap();
    let new: serde_json::Value = serde_json::from_str(zh).unwrap();
    assert_eq!(old["readings"], new["readings"]);
    assert_eq!(new["readings"].as_array().unwrap().len(), 3442);
    assert_eq!(new["continuations"].as_array().unwrap().len(), 906);
    let mut restored = new.clone();
    restored["continuations"][896] = serde_json::json!([
        "收纳袋",
        ["收纳袋放在衣柜下面那层。", "收纳袋里还放着帽子和围巾。"]
    ]);
    restored["continuations"][903] = serde_json::json!([
        "卡住",
        [
            "卡住的地方我先记下来，等你有空再一起看。",
            "卡住了也没关系，我们先确认缺的是哪条信息。"
        ]
    ]);
    assert_eq!(restored, old);
    assert_eq!(
        new["continuations"][896],
        serde_json::json!(["耳塞", AUTHORS])
    );
    assert_eq!(
        new["continuations"][903],
        serde_json::json!(["未完成", PROGRESS_AUTHORS])
    );
    let storage: serde_json::Value = serde_json::from_str(&storage_only(zh)).unwrap();
    let progress: serde_json::Value = serde_json::from_str(&progress_only(zh)).unwrap();
    let mut expected_storage = new.clone();
    expected_storage["continuations"][896] = old["continuations"][896].clone();
    assert_eq!(storage, expected_storage);
    let mut expected_progress = new.clone();
    expected_progress["continuations"][903] = old["continuations"][903].clone();
    assert_eq!(progress, expected_progress);
    let roundtrip = previous
        .replacen("      \"收纳袋\",\n      [", "      \"耳塞\",\n      [", 1)
        .replacen("收纳袋放在衣柜下面那层。", AUTHORS[0], 1)
        .replacen("收纳袋里还放着帽子和围巾。", AUTHORS[1], 1)
        .replacen("      \"卡住\",\n      [", "      \"未完成\",\n      [", 1)
        .replacen(
            "卡住的地方我先记下来，等你有空再一起看。",
            PROGRESS_AUTHORS[0],
            1,
        )
        .replacen(
            "卡住了也没关系，我们先确认缺的是哪条信息。",
            PROGRESS_AUTHORS[1],
            1,
        );
    assert_eq!(roundtrip, zh);
}

#[test]
fn release_storage_rechecks_all_eight_frozen_restock_pools_without_first_failure_shadowing() {
    let mut failures = Vec::new();
    for (seed, expected) in [
        ("shou na", 0x7b6db572bf58ae7b),
        ("shou na he", 0x83fa9a085c8d95b5),
        ("fang hui yuan chu", 0x9caa0daa67f164cb),
        ("yong wan fang hui yuan chu", 0x95443aa11dd56377),
        ("bang wo shou qi lai", 0x5d84ac6b18206c1a),
        ("wo tui chu chong jin yi xia", 0x50a5a7c697fc0f41),
        ("wo gang shang che", 0xcf339971e1d8c64d),
        ("ni de hua mian ka zhu le", 0xfbcc5afac78e49c8),
    ] {
        let ime = engine(seed);
        let frame = ime.candidates();
        let actual = pool_hash(frame);
        let valid = ime.snapshot().seed_text == seed
            && ime.snapshot().committed_text.is_empty()
            && frame.len() <= PAGE_SIZE * 2
            && frame.iter().all(|candidate| {
                candidate.source == CandidateSource::Local
                    && candidate.label == candidate.text
                    && candidate.score.is_finite()
            })
            && frame
                .iter()
                .map(|candidate| &candidate.text)
                .collect::<HashSet<_>>()
                .len()
                == frame.len();
        if actual != expected || !valid {
            failures.push(format!(
                "{seed:?}: expected={expected:016x} actual={actual:016x} valid={valid} frame={frame:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "all eight pools were checked:\n{}",
        failures.join("\n")
    );
}

#[test]
fn release_storage_new_words_four_authors_and_literals_commit_exactly_once() {
    for (seeds, word, authors) in [
        (["er'sai", "er'sa", "er3'sai1"], "耳塞", AUTHORS),
        (
            ["wei'wan'cheng", "wei'wan'chen", "wei4'wan2'cheng2"],
            "未完成",
            PROGRESS_AUTHORS,
        ),
    ] {
        for seed in seeds {
            for (text, kind) in [
                (word, CandidateKind::Word),
                (authors[0], CandidateKind::Sentence),
                (authors[1], CandidateKind::Sentence),
                (seed, CandidateKind::Literal),
            ] {
                let mut ime = engine(seed);
                assert!(ime.candidates().len() <= PAGE_SIZE * 2);
                let position = ime
                    .candidates()
                    .iter()
                    .position(|candidate| {
                        candidate.text == text
                            && candidate.kind == kind
                            && candidate.source == CandidateSource::Local
                    })
                    .unwrap_or_else(|| {
                        panic!("missing {seed:?}/{kind:?}/{text:?}: {:?}", ime.candidates())
                    });
                ime.select_candidate(position);
                assert_eq!(ime.snapshot().seed_text, seed);
                assert!(ime.snapshot().committed_text.is_empty());
                assert_eq!(ime.selected_completion_text(true), Some(text));
                let result = ime.commit(CommitOptions { force: true });
                assert!(result.ok);
                assert_eq!(result.text.as_deref(), Some(text));
                assert_eq!(ime.snapshot().committed_text, text);
                assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
                let again = ime.commit(CommitOptions { force: true });
                assert!(!again.ok && again.text.is_none());
                assert!(ime.undo().unwrap().committed_text.is_empty());
                assert!(ime.undo().is_none());
            }
        }
    }
    for (seed, text) in [("shou'na'dai", "收纳袋"), ("ka'zhu", "卡住")] {
        assert!(
            engine(seed)
                .candidates()
                .iter()
                .any(|candidate| candidate.text == text && candidate.kind == CandidateKind::Word)
        );
    }
}

#[test]
fn release_storage_inverse_preserves_other_languages_future_tails_and_strict_slot_readings_guards()
{
    // Preserve this release's exact 906-context snapshot, not runtime candidates.
    let published = presence_resource::without_presence(ZH);
    let zh = published.as_str();
    for raw in [
        include_str!("../data/lexicons/en.json"),
        include_str!("../data/lexicons/ja.json"),
    ] {
        assert_eq!(storage_only(raw), raw);
        assert_eq!(progress_only(raw), raw);
        assert_eq!(restore(raw), raw);
    }
    let current: serde_json::Value = serde_json::from_str(zh).unwrap();
    let base = current["continuations"].clone();
    let mut extra = base.clone();
    extra
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!(["以后追加", ["仅用于原文保真检查。"]]));
    let future_old: serde_json::Value =
        serde_json::from_str(&restore(&with_contexts(zh, &extra))).unwrap();
    assert_eq!(
        future_old["continuations"].as_array().unwrap().last(),
        extra.as_array().unwrap().last()
    );
    let mut unrelated = base.clone();
    unrelated[0][1][0] = "旧语境被篡改，必须由调用方完整旧hash发现。".into();
    assert_ne!(sha(&restore(&with_contexts(zh, &unrelated))), BEFORE_SHA);
    let mut invalid = Vec::new();
    let mut contexts = base.clone();
    contexts.as_array_mut().unwrap().pop();
    invalid.push(with_contexts(zh, &contexts));
    for slot in [896, 903] {
        let mut contexts = base.clone();
        contexts.as_array_mut().unwrap().swap(slot - 1, slot);
        invalid.push(with_contexts(zh, &contexts));
        let mut contexts = base.clone();
        contexts.as_array_mut().unwrap().push(base[slot].clone());
        invalid.push(with_contexts(zh, &contexts));
        let mut contexts = base.clone();
        contexts[slot][1].as_array_mut().unwrap().pop();
        invalid.push(with_contexts(zh, &contexts));
        let mut contexts = base.clone();
        contexts[slot][1].as_array_mut().unwrap().swap(0, 1);
        invalid.push(with_contexts(zh, &contexts));
    }
    for text in ["耳塞", "收纳袋", "未完成", "卡住"] {
        let mut changed = current.clone();
        let entry = changed["readings"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entry| entry["text"] == text)
            .unwrap();
        entry["kind"] = "sentence".into();
        invalid.push(serde_json::to_string_pretty(&changed).unwrap());
    }
    invalid.push(zh.replacen("  \"readings\": [", "  \"readings\":  [", 1));
    invalid.push("{\"language\":\"zh\"}".into());
    for raw in invalid {
        assert!(
            std::panic::catch_unwind(|| restore(&raw)).is_err(),
            "unknown delta accepted"
        );
    }
}

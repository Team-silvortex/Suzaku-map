//! Actual before/after replay: one reviewed second-page Word, not refreshed old goldens.
#[path = "support/farewell_replay.rs"]
mod farewell_replay;
use std::collections::HashSet;
use suzaku_map::ime::{
    Candidate, EngineConfig, XRTabletImeEngine,
    candidate_mix::{CandidateKind, CandidateSource},
};

fn corpus() -> Vec<String> {
    // Exact English corpus shared by both existing complete-vector replays.
    let phrases = [
        "i still have the receipt",
        "please let me know the address",
        "i've taken photos of the damage",
        "could you check the opening hours",
    ];
    let mut seeds = Vec::new();
    for phrase in phrases {
        for end in phrase.char_indices().map(|(i, ch)| i + ch.len_utf8()) {
            seeds.push(phrase[..end].to_owned());
        }
    }
    for seed in [
        "",
        "https://example.invalid/hoshousho",
        "abc_xyz",
        "xyzunknown",
        "one\nline",
        "わからない",
        "包装",
        "保证書",
        "i",
        "p",
        "c",
    ] {
        seeds.push(seed.to_owned());
    }
    seeds.push(format!("{} {}", "既存の文章🙂 ".repeat(40), phrases[0]));
    seeds
}

fn fixture(rows: &[(&str, &str, u32)]) -> Vec<Candidate> {
    rows.iter()
        .map(|&(text, kind, bits)| Candidate {
            text: text.to_owned(),
            label: text.to_owned(),
            score: f32::from_bits(bits),
            kind: CandidateKind::parse(kind).unwrap(),
            source: CandidateSource::Local,
        })
        .collect()
}

// Independent literals transcribed from the two actual diagnostic executions,
// not read from the production resource or the replay helper's expectations.
fn after() -> Vec<Candidate> {
    fixture(&[
        ("i s", "literal", 1120403456),
        ("i should've", "word", 1119354880),
        ("i see", "word", 1119092736),
        ("i still", "word", 1118830592),
        ("i should've called earlier.", "sentence", 1117650944),
        ("i should've checked the time.", "sentence", 1117585408),
        ("i should", "word", 1118568448),
        ("i see what you mean.", "sentence", 1117519872),
        ("i see what you need now.", "sentence", 1117454336),
        (
            "i still have the receipt if you need to see it.",
            "sentence",
            1117388800,
        ),
        (
            "i still have the box and the original packaging.",
            "sentence",
            1117323264,
        ),
    ])
}

fn before() -> Vec<Candidate> {
    fixture(&[
        ("i s", "literal", 1120403456),
        ("i should've", "word", 1119354880),
        ("i see", "word", 1119092736),
        ("i still", "word", 1118830592),
        ("i should've called earlier.", "sentence", 1117650944),
        ("i should've checked the time.", "sentence", 1117585408),
        ("i see what you mean.", "sentence", 1117519872),
        ("i see what you need now.", "sentence", 1117454336),
        (
            "i still have the receipt if you need to see it.",
            "sentence",
            1117388800,
        ),
        (
            "i still have the box and the original packaging.",
            "sentence",
            1117323264,
        ),
    ])
}

fn hash_vector(hash: &mut u64, seed: &str, candidates: &[Candidate]) {
    for byte in seed.bytes().chain([0]) {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
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
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
}

#[test]
fn farewell_english_replay_accepts_only_one_verified_added_word_across_130_vectors() {
    let seeds = corpus();
    assert_eq!(seeds.len(), 130);
    let mut ime = XRTabletImeEngine::new(EngineConfig::default());
    ime.enable_ibus_candidate_mix();
    let (mut current_hash, mut previous_hash) = (0xcbf29ce484222325_u64, 0xcbf29ce484222325_u64);
    for (index, seed) in seeds.iter().enumerate() {
        ime.seed(seed);
        assert_eq!(ime.snapshot().seed_text, *seed);
        assert!(ime.snapshot().committed_text.is_empty());
        let current = ime.candidates();
        assert!(current.len() <= 12);
        assert_eq!(
            current.len(),
            current
                .iter()
                .map(|row| &row.text)
                .collect::<HashSet<_>>()
                .len()
        );
        let previous = farewell_replay::before_farewell("en", seed, current, false);
        if index == 2 {
            assert_eq!(seed, "i s");
            assert_eq!(current, after());
            assert_eq!(previous, before());
        } else {
            assert_eq!(
                previous, current,
                "projection changed another seed: {seed:?}"
            );
        }
        hash_vector(&mut current_hash, seed, current);
        hash_vector(&mut previous_hash, seed, &previous);
    }
    // Raw vectors, including existing vocabulary deltas such as one\nline.
    // These are actual batch-before/batch-after hashes, not the older
    // optimization fingerprint and not output from any historical projection.
    assert_eq!(current_hash, 0xcba920c9edee8a37);
    assert_eq!(previous_hash, 0x773ce9b32e1ddf34);
}

fn rejects(current: &[Candidate]) {
    assert!(
        std::panic::catch_unwind(|| farewell_replay::before_farewell("en", "i s", current, false))
            .is_err(),
        "the exact-scope helper accepted a changed complete vector"
    );
}

#[test]
fn farewell_replay_guard_rejects_mutations_and_leaves_other_inputs_unprojected() {
    let current = after();
    assert_eq!(
        farewell_replay::before_farewell("en", "i s", &current, false),
        before()
    );
    for index in 0..current.len() {
        for field in 0..5 {
            let mut changed = current.clone();
            let row = &mut changed[index];
            match field {
                0 => row.text.push_str("<changed>"),
                1 => row.label.push_str("<changed>"),
                2 => {
                    row.kind = if row.kind == CandidateKind::Word {
                        CandidateKind::Literal
                    } else {
                        CandidateKind::Word
                    };
                }
                3 => row.source = CandidateSource::Model,
                _ => row.score = f32::from_bits(row.score.to_bits() + 1),
            }
            rejects(&changed);
        }
        let mut changed = current.clone();
        changed.remove(index);
        rejects(&changed);
    }
    for index in 0..current.len() - 1 {
        let mut changed = current.clone();
        changed.swap(index, index + 1);
        rejects(&changed);
    }
    for index in 0..=current.len() {
        let mut changed = current.clone();
        changed.insert(index, current[6].clone());
        rejects(&changed);
    }
    let mut changed = current.clone();
    changed[6].label = "unrelated".into();
    for rows in [&current, &changed, &Vec::new()] {
        for language in ["", "EN", "en-US", "zh-Hans", "ja"] {
            assert_eq!(
                farewell_replay::before_farewell(language, "i s", rows, false),
                *rows
            );
        }
        for seed in ["", "i", "i s ", " i s", "I s", "i\ts", "i sh", "i s\n"] {
            assert_eq!(
                farewell_replay::before_farewell("en", seed, rows, false),
                *rows
            );
        }
        assert_eq!(
            farewell_replay::before_farewell("en", "i s", rows, true),
            *rows
        );
    }
}

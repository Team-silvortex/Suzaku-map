//! Deterministic synthetic input replay. No native keys, personal data or models.
//! Run separate processes for builtin and --ja-packs startup snapshots.
use std::{
    fs,
    hint::black_box,
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use suzaku_map::{
    ime::{EngineConfig, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn packs_fixture() -> Fixture {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("suzaku-input-bench-{}-{nonce}", std::process::id()));
    fs::create_dir(&path).unwrap();
    let fixture = Fixture(path);
    let store = PackStore::new(fixture.0.join("packs")).unwrap();
    let catalog = packs::recommended();
    let selected: Vec<_> = catalog
        .iter()
        .filter(|p| p.manifest.language == "ja")
        .collect();
    assert_eq!(selected.len(), 9);
    for pack in selected {
        store.install(pack.clone(), false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), 9);
    fixture
}

fn corpus(language: &str) -> Vec<String> {
    let phrases: &[&str] = match language {
        "en" => &[
            "i still have the receipt",
            "please let me know the address",
            "i've taken photos of the damage",
            "could you check the opening hours",
        ],
        "zh-Hans" => &[
            "gou mai ping zheng zen me ti gong",
            "shou hou jin du bang wo cha yi xia",
            "na guo lai",
            "zhe ben shu gei ni",
        ],
        "ja" => &[
            "nihongo wo benkyou shite",
            "hoshousho",
            "henkinn",
            "seirikenn",
            "kyouyu",
            "jyunbi gade ki",
            "konnpouzai",
        ],
        _ => unreachable!(),
    };
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

fn engine(language: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime
}

fn fingerprint(ime: &mut XRTabletImeEngine, seeds: &[String]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for seed in seeds {
        ime.seed(seed);
        for byte in seed.bytes().chain([0]) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        for candidate in ime.candidates() {
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
    }
    hash
}

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        arguments
            .iter()
            .all(|a| matches!(a.as_str(), "--ja-packs" | "--vectors")),
        "Only --ja-packs and --vectors are supported"
    );
    let with_packs = arguments.iter().any(|a| a == "--ja-packs");
    let vectors = arguments.iter().any(|a| a == "--vectors");
    let _fixture = with_packs.then(packs_fixture);
    let languages: &[&str] = if with_packs {
        &["ja"]
    } else {
        &["en", "zh-Hans", "ja"]
    };
    for &language in languages {
        let seeds = corpus(language);
        let mut ime = engine(language);
        let fingerprint = fingerprint(&mut ime, &seeds);
        if vectors {
            for seed in &seeds {
                ime.seed(seed);
                let candidates: Vec<_> = ime
                    .candidates()
                    .iter()
                    .map(|c| {
                        serde_json::json!([
                            c.text,
                            c.label,
                            c.kind.id(),
                            c.source.id(),
                            c.score.to_bits()
                        ])
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({"language": language, "seed": seed, "candidates": candidates})
                );
            }
            continue;
        }
        // Warm caches before timing; candidates and startup-pack order are fixed.
        for _ in 0..2 {
            for seed in &seeds {
                black_box(ime.seed(black_box(seed)));
            }
        }
        let mut samples = Vec::with_capacity(seeds.len() * 30);
        for _ in 0..30 {
            for seed in &seeds {
                let started = Instant::now();
                black_box(ime.seed(black_box(seed)));
                samples.push(started.elapsed().as_nanos() as u64);
            }
        }
        let mean = samples.iter().map(|&n| u128::from(n)).sum::<u128>() / samples.len() as u128;
        samples.sort_unstable();
        println!(
            "{}",
            serde_json::json!({
                "language": language, "mode": if with_packs { "nine-ja-packs" } else { "builtin" },
                "seeds": seeds.len(), "iterations": 30, "samples": samples.len(),
                "mean_us": mean as f64 / 1000.0,
                "p50_us": samples[samples.len()/2] as f64 / 1000.0,
                "p95_us": samples[samples.len()*95/100] as f64 / 1000.0,
                "fingerprint": format!("{fingerprint:016x}")
            })
        );
    }
}

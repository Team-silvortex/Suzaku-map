//! Explicit compatibility cohorts, not an automatic installation policy.
//! Available collections may outnumber the unchanged sixteen installed slots.
use suzaku_map::lexicon::packs::{self, OfflinePack};

const HISTORICAL_IDS: [&str; 16] = [
    "org.suzaku.en.outdoors",
    "org.suzaku.zh-hans.outdoors",
    "org.suzaku.ja.rail",
    "org.suzaku.en.study",
    "org.suzaku.zh-hans.study",
    "org.suzaku.en.cooking",
    "org.suzaku.zh-hans.cooking",
    "org.suzaku.en.travel",
    "org.suzaku.zh-hans.travel",
    "org.suzaku.en.work",
    "org.suzaku.zh-hans.work",
    "org.suzaku.en.family",
    "org.suzaku.zh-hans.family",
    "org.suzaku.ja.family",
    "org.suzaku.en.shopping",
    "org.suzaku.zh-hans.shopping",
];

pub fn historical_sixteen() -> Vec<OfflinePack> {
    let catalog = packs::recommended();
    assert!(catalog.len() >= HISTORICAL_IDS.len());
    assert_eq!(
        catalog[..HISTORICAL_IDS.len()]
            .iter()
            .map(|pack| pack.manifest.id.as_str())
            .collect::<Vec<_>>(),
        HISTORICAL_IDS,
        "historical compatibility cohort must retain every original pack in order"
    );
    catalog.into_iter().take(HISTORICAL_IDS.len()).collect()
}

#[allow(dead_code)]
pub fn historical_eighteen() -> Vec<OfflinePack> {
    let mut expected = HISTORICAL_IDS.to_vec();
    expected.extend(["org.suzaku.en.internet", "org.suzaku.zh-hans.internet"]);
    let catalog = packs::recommended();
    assert!(catalog.len() >= expected.len());
    assert_eq!(
        catalog[..expected.len()]
            .iter()
            .map(|pack| pack.manifest.id.as_str())
            .collect::<Vec<_>>(),
        expected,
        "the historical eighteen-pack catalog prefix must not change"
    );
    catalog.into_iter().take(expected.len()).collect()
}

// Only the shared quality target needs the supplemental cohort. Other targets
// still execute their complete historical sixteen-pack expectations unchanged.
#[allow(dead_code)]
pub fn with_internet() -> Vec<OfflinePack> {
    let mut selected = historical_sixteen();
    selected.truncate(14);
    let catalog = packs::recommended();
    for id in ["org.suzaku.en.internet", "org.suzaku.zh-hans.internet"] {
        selected.push(
            catalog
                .iter()
                .find(|pack| pack.manifest.id == id)
                .unwrap_or_else(|| panic!("missing required supplemental collection {id}"))
                .clone(),
        );
    }
    assert_eq!(selected.len(), 16);
    selected
}

// Keep all fourteen original packs required by the shared quality assertions.
// The separate Japanese suite checks all three internet packs together.
#[allow(dead_code)]
pub fn with_japanese_internet() -> Vec<OfflinePack> {
    let mut selected = with_internet();
    assert_eq!(
        selected.pop().unwrap().manifest.id,
        "org.suzaku.zh-hans.internet"
    );
    selected.push(
        packs::recommended()
            .into_iter()
            .find(|pack| pack.manifest.id == "org.suzaku.ja.internet")
            .expect("missing required Japanese internet collection"),
    );
    assert_eq!(selected.len(), 16);
    selected
}

#[allow(dead_code)]
pub fn historical_nineteen() -> Vec<OfflinePack> {
    let mut expected: Vec<_> = historical_eighteen()
        .into_iter()
        .map(|pack| pack.manifest.id)
        .collect();
    expected.push("org.suzaku.ja.internet".to_owned());
    let catalog = packs::recommended();
    assert!(catalog.len() >= expected.len());
    assert_eq!(
        catalog[..expected.len()]
            .iter()
            .map(|p| &p.manifest.id)
            .collect::<Vec<_>>(),
        expected.iter().collect::<Vec<_>>()
    );
    catalog.into_iter().take(expected.len()).collect()
}

#[allow(dead_code)]
pub fn historical_twenty_two() -> Vec<OfflinePack> {
    let mut expected: Vec<_> = historical_nineteen()
        .into_iter()
        .map(|pack| pack.manifest.id)
        .collect();
    expected.extend(
        [
            "org.suzaku.ja.shopping",
            "org.suzaku.ja.travel",
            "org.suzaku.ja.work",
        ]
        .map(str::to_owned),
    );
    let catalog = packs::recommended();
    assert!(catalog.len() >= expected.len());
    assert_eq!(
        catalog[..expected.len()]
            .iter()
            .map(|p| &p.manifest.id)
            .collect::<Vec<_>>(),
        expected.iter().collect::<Vec<_>>()
    );
    catalog.into_iter().take(expected.len()).collect()
}

// Original fourteen remain present for every unchanged shared assertion.
#[allow(dead_code)]
pub fn with_japanese_theme(id: &str) -> Vec<OfflinePack> {
    assert!(
        [
            "org.suzaku.ja.shopping",
            "org.suzaku.ja.travel",
            "org.suzaku.ja.work",
            "org.suzaku.ja.study",
            "org.suzaku.ja.cooking",
            "org.suzaku.ja.outdoors"
        ]
        .contains(&id)
    );
    let mut selected = with_japanese_internet();
    assert_eq!(selected.remove(14).manifest.id, "org.suzaku.en.internet");
    selected.push(
        packs::recommended()
            .into_iter()
            .find(|p| p.manifest.id == id)
            .unwrap_or_else(|| panic!("missing required Japanese theme {id}")),
    );
    assert_eq!(selected.len(), 16);
    selected
}

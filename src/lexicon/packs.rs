//! Data-only offline packs. Installation is transactional; activation is a
//! bounded, immutable startup snapshot, independent of hosts and model APIs.
mod catalog;
mod format;
mod runtime;
mod store;

pub use catalog::{Catalog, LoadReport};
pub use format::{InstalledPack, Manifest, OfflinePack, PackStats};
pub use runtime::{initialize, initialize_current, runtime};
pub use store::PackStore;

pub const MAX_PACK_BYTES: usize = 256 * 1024;
pub const MAX_PACK_UNITS: usize = 2048;
pub const MAX_LANGUAGE_UNITS: usize = 4096;
pub const MAX_PACKS: usize = 16;

/// Small authored collections shipped with the tool; browsing/exporting never
/// downloads, installs or enables anything. Topics are stable filter tags.
pub fn recommended() -> Vec<OfflinePack> {
    [
        include_str!("../../data/offline-packs/en-outdoors.json"),
        include_str!("../../data/offline-packs/zh-Hans-outdoors.json"),
        include_str!("../../data/offline-packs/ja-rail.json"),
        include_str!("../../data/offline-packs/en-study.json"),
        include_str!("../../data/offline-packs/zh-Hans-study.json"),
        include_str!("../../data/offline-packs/en-cooking.json"),
        include_str!("../../data/offline-packs/zh-Hans-cooking.json"),
        include_str!("../../data/offline-packs/en-travel.json"),
        include_str!("../../data/offline-packs/zh-Hans-travel.json"),
        include_str!("../../data/offline-packs/en-work.json"),
        include_str!("../../data/offline-packs/zh-Hans-work.json"),
        include_str!("../../data/offline-packs/en-family.json"),
        include_str!("../../data/offline-packs/zh-Hans-family.json"),
        include_str!("../../data/offline-packs/ja-family.json"),
        include_str!("../../data/offline-packs/en-shopping.json"),
        include_str!("../../data/offline-packs/zh-Hans-shopping.json"),
        include_str!("../../data/offline-packs/en-internet.json"),
        include_str!("../../data/offline-packs/zh-Hans-internet.json"),
        include_str!("../../data/offline-packs/ja-internet.json"),
        include_str!("../../data/offline-packs/ja-shopping.json"),
        include_str!("../../data/offline-packs/ja-travel.json"),
        include_str!("../../data/offline-packs/ja-work.json"),
        include_str!("../../data/offline-packs/ja-study.json"),
        include_str!("../../data/offline-packs/ja-cooking.json"),
        include_str!("../../data/offline-packs/ja-outdoors.json"),
    ]
    .into_iter()
    .map(|raw| OfflinePack::from_json(raw).expect("bundled offline pack must pass validation"))
    .collect()
}

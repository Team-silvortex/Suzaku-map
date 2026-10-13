//! One immutable startup snapshot shared by the process's language indexes.
use super::catalog::{Catalog, LoadReport};
use super::store::PackStore;
use std::sync::OnceLock;

static ACTIVE: OnceLock<Catalog> = OnceLock::new();

pub fn runtime() -> &'static Catalog {
    ACTIVE.get_or_init(Catalog::default)
}

/// Call before constructing any engine. Late reinitialization is an error, not
/// a partial reload underneath already-created language indexes.
pub fn initialize(store: &PackStore) -> Result<&'static LoadReport, String> {
    if ACTIVE.get().is_some() {
        return Err("offline vocabulary is already frozen; restart the process".into());
    }
    ACTIVE
        .set(store.load())
        .map_err(|_| "offline vocabulary is already frozen; restart the process".to_owned())?;
    Ok(&runtime().report)
}

/// Host entry point: damaged/unavailable packs never stop built-in input.
pub fn initialize_current() {
    match PackStore::current().and_then(|store| initialize(&store)) {
        Ok(report) => {
            for error in &report.errors {
                eprintln!("Suzaku offline pack skipped: {error}");
            }
            if !report.loaded.is_empty() {
                eprintln!("Suzaku offline packs loaded: {}", report.loaded.join(", "));
            }
        }
        Err(error) => eprintln!("Suzaku offline packs unavailable: {error}"),
    }
}

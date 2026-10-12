#[cfg(target_os = "linux")]
fn main() {
    let _data_lease = match suzaku_map::data::files::DataLease::current_shared() {
        Ok(lease) => lease,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    suzaku_map::lexicon::packs::initialize_current();
    suzaku_map::ime_host::persistence::initialize();
    let result = suzaku_map::platform::linux_ibus_host::run();
    suzaku_map::ime_host::persistence::shutdown();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("The native IBus host is only available on Linux.");
    std::process::exit(1);
}

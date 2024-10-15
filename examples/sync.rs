use quickfetch::package::Mode;
use quickfetch::package::SimplePackage;
use quickfetch::Fetcher;

fn main() -> anyhow::Result<()> {
    quickfetch::pretty_env_logger::init();
    let config_path = "examples/pkgs.toml";
    let mode = Mode::Toml;

    let mut syncfetch: Fetcher<SimplePackage> = Fetcher::new_sync(config_path, mode, "sf")?;

    syncfetch.sync_fetch()?;

    Ok(())
}

use std::time::Instant;

use quickfetch::package::Mode;
use quickfetch::package::SimplePackage;
use quickfetch::Fetcher;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    quickfetch::pretty_env_logger::init();
    let config_path = "examples/pkgs.toml";
    let mode = Mode::Toml;

    let mut fetcher1: Fetcher<SimplePackage> = Fetcher::new(config_path, mode, "timer_1").await?;
    let mut fetcher2: Fetcher<SimplePackage> = Fetcher::new(config_path, mode, "timer_2").await?;

    let start = Instant::now();
    fetcher1.concurrent_fetch().await?;
    let duration = start.elapsed();
    println!("Time elapsed for fetcher1: {:?}", duration);

    let start = Instant::now();
    fetcher2.sync_fetch()?;
    let duration = start.elapsed();
    println!("Time elapsed for fetcher2: {:?}", duration);

    Ok(())
}

use std::time::Instant;

use quickfetch::package::{Config, SimplePackage};
use quickfetch::Fetcher;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    quickfetch::pretty_env_logger::init();
    let config: Config<SimplePackage> = Config::from_toml_file("examples/pkgs.toml").await?;

    let mut fetcher1 = Fetcher::new(config.packages(), "timer_1")?;
    let mut fetcher2 = Fetcher::new(config.packages(), "timer_2")?;

    let start = Instant::now();
    fetcher1.concurrent_fetch().await?;
    let duration = start.elapsed();
    println!("Time elapsed for fetcher1: {:?}", duration);

    let start = Instant::now();
    fetcher2.channel_fetch().await?;
    let duration = start.elapsed();
    println!("Time elapsed for fetcher2: {:?}", duration);

    Ok(())
}

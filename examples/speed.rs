#![allow(unused_imports)]
use std::path::PathBuf;

use quickfetch::package::SimplePackage;
use quickfetch::Fetcher;
use quickfetch::{pretty_env_logger, FetchMethod};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let config_path = "examples/pkgs.toml";

    tokio::fs::remove_dir_all("mufiz").await?;
    //tokio::fs::remove_dir_all("pkgs").await?;

    let start = std::time::Instant::now();
    let mut fetcher: Fetcher<SimplePackage> =
        Fetcher::new(config_path, quickfetch::package::Mode::Toml, "mufiz").await?;
    // Set the response method to BytesStream or Chunk for progress bars
    fetcher.set_response_method(quickfetch::ResponseMethod::Chunk);
    // To enable progress bar for fetching
    fetcher.set_notify_method(quickfetch::NotifyMethod::Silent);
    // Fetch the packages asynchronously
    fetcher.fetch(FetchMethod::Async).await?;
    let elapsed = start.elapsed();

    println!("Time: {}s", elapsed.as_secs());

    Ok(())
}

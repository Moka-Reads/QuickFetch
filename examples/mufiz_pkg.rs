#![allow(unused_imports)]
use std::path::PathBuf;

use quickfetch::package::SimplePackage;
use quickfetch::Fetcher;
use quickfetch::{pretty_env_logger, FetchMethod};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let config_path = "examples/pkgs.toml";

    let mut fetcher: Fetcher<SimplePackage> =
        Fetcher::new(config_path, quickfetch::package::Mode::Toml, "mufiz").await?;
    // Set the response method to BytesStream or Chunk for progress bars
    fetcher.set_response_method(quickfetch::ResponseMethod::Chunk);
    // To enable progress bar for fetching
    fetcher.set_notify_method(quickfetch::NotifyMethod::ProgressBar);
    // Fetch the packages asynchronously
    fetcher.fetch(FetchMethod::Async).await?;
    // Write the fetched packages to a directory
    fetcher.write_all(PathBuf::from("pkgs")).await?;

    // To enable watching
    // fetcher.watching().await;

    Ok(())
}

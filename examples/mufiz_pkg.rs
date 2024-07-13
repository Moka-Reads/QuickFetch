#![allow(unused_imports)]
use std::path::PathBuf;

use quickfetch::encryption::AESGCM;
use quickfetch::package::SimplePackage;
use quickfetch::Fetcher;
use quickfetch::{pretty_env_logger, FetchMethod};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let config_path = "examples/watch.toml";

    let mut fetcher: Fetcher<SimplePackage, AESGCM> = Fetcher::new(
        config_path,
        quickfetch::package::Mode::Toml,
        "mufiz",
        AESGCM,
    )
    .await?;
    // To enable progress bar for fetching
    // fetcher.set_notify_method(quickfetch::NotifyMethod::ProgressBar);
    // Set the response method to BytesStream or Chunk for progress bars
    // fetcher.set_response_method(quickfetch::ResponseMethod::BytesStream);
    // Fetch the packages concurrently
    // fetcher.fetch(FetchMethod::Concurrent).await?;
    // Write the fetched packages to a directory
    // fetcher.write_all(PathBuf::from("pkgs")).await?;

    // To enable watching
    fetcher.watching().await;

    Ok(())
}

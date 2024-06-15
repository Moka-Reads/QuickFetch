use std::path::PathBuf;

use quickfetch::package::{Config, Package};
use quickfetch::Fetcher;
use quickfetch::{pretty_env_logger, FetchMethod};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let config: Config<Package> = Config::from_toml_file("examples/pkgs.toml").await?;

    let mut fetcher: Fetcher<Package> = Fetcher::new(config.packages(), "mufiz")?;
    fetcher.fetch(FetchMethod::Channel).await?;
    fetcher.write_all(PathBuf::from("pkgs")).await?;
    Ok(())
}

use std::path::PathBuf;

use quickfetch::package::{Config, Package};
use quickfetch::pretty_env_logger;
use quickfetch::Fetcher;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let config: Config<Package> = Config::from_toml_file("examples/pkgs.toml").await?;

    let mut fetcher: Fetcher<Package> = Fetcher::new(config.packages(), "mufiz")?;
    fetcher.fetch().await?;
    fetcher.write_all(PathBuf::from("pkgs")).await?;
    Ok(())
}

use std::path::PathBuf;
use tokio::fs::read_to_string;
use toml::from_str;
use quickfetch::pretty_env_logger;
use quickfetch::package::{Config, Package};
use quickfetch::Fetcher;

#[tokio::main]
async fn main() -> anyhow::Result<()>{
    pretty_env_logger::init();
    let content = read_to_string("pkgs.toml").await?;
    let packages: Config = from_str(&content).unwrap();

    let mut fetcher: Fetcher<Package> = Fetcher::new(&packages.packages(), "mufiz")?;
    fetcher.fetch().await?;
    fetcher.write_all(PathBuf::from("pkgs")).await?;
    Ok(())
}
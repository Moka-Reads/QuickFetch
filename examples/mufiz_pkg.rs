use std::path::PathBuf;

use quickfetch::package::{Config, SimplePackage};
use quickfetch::Fetcher;
use quickfetch::{pretty_env_logger, FetchMethod};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let config: Config<SimplePackage> = Config::from_toml_file("examples/pkgs.toml").await?;

    let mut fetcher: Fetcher<SimplePackage> = Fetcher::new(config.packages(), "mufiz")?;
    fetcher.set_notify_method(quickfetch::NotifyMethod::ProgressBar);
    fetcher.set_response_method(quickfetch::ResponseMethod::BytesStream);
    fetcher.fetch(FetchMethod::Concurrent).await?;
    fetcher.write_all(PathBuf::from("pkgs")).await?;
    Ok(())
}

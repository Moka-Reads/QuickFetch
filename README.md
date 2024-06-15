![QuickFetch logo](QuickFetch.png)

## A Library to Fetch well Quickly...

> :warning: WORK IN PROGRESS AND NOT READY TO BE USED FOR PRODUCTION YET

This library is built to handle multiple requests within a `Client` (`reqwest` client which will handle it all under a Client Pool)
, cache the response results, and handle these in parallel and asynchronously. 

The goal is to be a one-stop shop for handling local package manager development to handle multiple 
packages with a local cache to easily update, get and remove the different responses.


## Usage

```rust
use std::path::PathBuf;

use quickfetch::package::{Config, Package};
use quickfetch::pretty_env_logger;
use quickfetch::Fetcher;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let config: Config<Package> = Config::from_toml_file("examples/pkgs.toml").await?;

    let mut fetcher: Fetcher<Package> = Fetcher::new(config.packages(), "mufiz")?;
    fetcher.fetch(Default::default()).await?;
    fetcher.write_all(PathBuf::from("pkgs")).await?;
    Ok(())
}
```
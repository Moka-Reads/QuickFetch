# QuickFetch

## A Library to Fetch well Quickly...

Developed by Mustafif Khan | MoKa Reads 2024

**This library is licensed under the MIT License**

> :warning: WORK IN PROGRESS AND NOT READY TO BE USED FOR PRODUCTION YET

This library is built to handle multiple requests within a `Client` (`reqwest` client which will handle it all under a Client Pool)
, cache the response results, and handle these in parallel and asynchronously.

The goal is to be a one-stop shop for handling local package manager development to handle multiple
packages with a local cache to easily update, get and remove the different responses.


## Usage

```rust
// examples/mufiz_pkg.rs
use std::path::PathBuf;

use quickfetch::package::{Config, SimplePackage};
use quickfetch::Fetcher;
use quickfetch::{pretty_env_logger, FetchMethod};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let config: Config<SimplePackage> = Config::from_toml_file("examples/pkgs.toml").await?;

    let mut fetcher: Fetcher<SimplePackage> = Fetcher::new(config.packages(), "mufiz")?;
    fetcher.fetch(FetchMethod::Concurrent).await?;
    fetcher.write_all(PathBuf::from("pkgs")).await?;
    Ok(())
}
```


## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

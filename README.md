# QuickFetch

## A Library to Fetch well Quickly...

Developed by Mustafif Khan | MoKa Reads 2024

> :warning: WORK IN PROGRESS AND NOT READY TO BE USED FOR PRODUCTION YET

This library is built to handle multiple requests within a `Client` (`reqwest` client which will handle it all under a Client Pool)
, cache the response results, and handle these asynchronously.

The goal is to be a one-stop shop for handling local package manager development to handle multiple
packages with a local cache to easily update, get and remove the different responses.


## Usage

```rust
// examples/mufiz_pkg.rs
use std::path::PathBuf;

use quickfetch::package::SimplePackage;
use quickfetch::Fetcher;
use quickfetch::{pretty_env_logger, FetchMethod};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let config_path = "examples/watch.toml";

    let mut fetcher: Fetcher<SimplePackage> =
        Fetcher::new(config_path, quickfetch::package::Mode::Toml, "mufiz").await?;
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
```


## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

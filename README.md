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
```

## Encryption Methods Available

All of these types are unit structs that can be found in the
`quickfetch::encryption` module.

- `AESGCM`:  AES GCM encryption method (default under the `aes` feature)
- `ChaCha20Poly` : ChaCha20 Poly1305 encryption method (under the `chacha20poly` feature)
- `AESGCMSIV`:  AES GCM SIV encryption method (under the `aes-gcm-siv` feature)
- `AESSIV` : AES SIV encryption method (under the `aes-siv` feature)
- `Ascon` : Ascon encryption method (under the `ascon-aead` feature)
- `CCM`: CCM encryption method (under the `ccm` feature)
- `Deoxys` : Deoxys encryption method (under the `deoxys` feature)
- `EAX` : EAX encryption method (under the `eax` feature)

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

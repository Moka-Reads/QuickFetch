use quickfetch::package::SimplePackage;
use quickfetch::watcher::WatchFetcher;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut wf: WatchFetcher<SimplePackage> = WatchFetcher::new(
        "watchdb",
        "examples/watch.toml",
        quickfetch::package::Mode::Toml,
    )
    .await?;

    wf.watching().await;

    Ok(())
}

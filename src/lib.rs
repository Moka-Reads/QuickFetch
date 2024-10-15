#![doc = include_str!("../README.md")]
#[macro_use]
extern crate log;
use anyhow::Result;
pub use bincode;
use bytes::Bytes;
use futures::future::join_all;
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use notify::{Config as NConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use package::{Config, Mode};
pub use pretty_env_logger;
pub use quickfetch_traits as traits;
use quickfetch_traits::{Entry, EntryKey, EntryValue};
use reqwest::{Client, Response};
use serde::Deserialize;
use sled::Db;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::create_dir;
use tokio::sync::mpsc::{channel, Receiver};
use tokio::sync::Mutex;
use url::Url;
/// Provides different types of packages that can be used
pub mod package;
/// Provides structures that can be used as a Key and Value for Fetcher
pub mod val;

/// Provides all the common types to use with Fetcher
pub mod prelude {
    pub use crate::package::{Config, GHPackage, Mode, SimplePackage};
    pub use crate::traits::{Entry, EntryKey, EntryValue};
    pub use crate::val::{GHValue, SimpleValue};
    pub use crate::Fetcher;
}

/// Returns the path to the home directory with the sub directory appended
pub fn home_plus<P: AsRef<Path>>(sub_dir: P) -> PathBuf {
    dirs::home_dir().unwrap().join(sub_dir)
}

/// Returns the path to the config directory with the sub directory appended
pub fn config_plus<P: AsRef<Path>>(sub_dir: P) -> PathBuf {
    dirs::config_dir().unwrap().join(sub_dir)
}

/// Returns the path to the cache directory with the sub directory appended
pub fn cache_plus<P: AsRef<Path>>(sub_dir: P) -> PathBuf {
    dirs::cache_dir().unwrap().join(sub_dir)
}

/// `ResponseMethod` enum to specify the method of fetching the response
///
/// - `Bytes`: Fetch the full response using the `bytes` method
/// - `Chunk`: Fetch the response in chunks using the `chunk` method
/// - `BytesStream`: Fetch the response in a stream of bytes using the `bytes_stream` method
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResponseMethod {
    Bytes,
    Chunk,
    BytesStream,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotifyMethod {
    Log,
    ProgressBar,
    Silent,
}

impl Default for NotifyMethod {
    fn default() -> Self {
        Self::Log
    }
}

/// `FetchMethod` enum to specify the method of fetching the response
///
/// - `Async`: Fetch the response asynchronously using `tokio::spawn`
/// - `Channel`: Fetch the response using a bounded multi-producer single-consumer channel
#[derive(Debug, Copy, Clone)]
pub enum FetchMethod {
    Async,
    Watch,
    #[cfg(feature = "unstable")]
    Sync,
}

impl Default for FetchMethod {
    fn default() -> Self {
        Self::Async
    }
}

impl Default for ResponseMethod {
    fn default() -> Self {
        Self::Bytes
    }
}

/// Fetcher struct that will be used to fetch and cache data
///
/// - `entries`: List of entries to fetch
/// - `db`: sled db to cache the fetched data
/// - `client`: reqwest client to fetch the data
/// - `response_method`: Method of fetching the response
/// - `encryption_method`: Method of encrypting and decrypting the response
#[derive(Debug, Clone)]
pub struct Fetcher<E: Entry> {
    /// List of entries to fetch
    entries: Arc<Vec<E>>,
    /// Path to the config file
    config_path: PathBuf,
    /// Config struct to hold the configuration
    config: Config<E>,
    /// Type of config file (json or toml)
    config_type: Mode,
    /// sled db to cache the fetched data
    db: Db,
    /// Path to the db file
    db_path: PathBuf,
    /// reqwest client to fetch the data
    client: Client,
    /// Method of fetching the response
    response_method: ResponseMethod,
    /// Method of notifying the user
    notify_method: NotifyMethod,
    /// Multi progress bar to show multiple progress bars
    multi_pb: Arc<MultiProgress>,
}

// Constructor and Setup Methods
impl<E: Entry + Clone + Send + Sync + 'static + for<'de> Deserialize<'de>> Fetcher<E> {
    /// Create a new `Fetcher` instance with list of urls and db path
    pub async fn new<P: AsRef<Path> + Send + Sync>(
        config_path: P,
        config_type: Mode,
        db_path: P,
    ) -> Result<Self> {
        let client = Client::builder()
            .brotli(true) // by default enable brotli decompression
            .build()?;

        let config = Config::from_file(&config_path, config_type).await?;
        let entries = config.packages_owned();

        Ok(Self {
            entries: Arc::new(entries),
            db: sled::open(&db_path)?,
            db_path: PathBuf::from(db_path.as_ref()),
            config,
            config_path: config_path.as_ref().to_path_buf(),
            config_type,
            client,
            response_method: ResponseMethod::default(),
            notify_method: NotifyMethod::Log,
            multi_pb: Arc::new(MultiProgress::new()),
        })
    }

    #[cfg(feature = "unstable")]
    /// Create a new `Fetcher` instance with list of urls and db path synchronously
    pub fn new_sync<P: AsRef<Path> + Send + Sync>(
        config_path: P,
        config_type: Mode,
        db_path: P,
    ) -> Result<Self> {
        futures::executor::block_on(Self::new(config_path, config_type, db_path))
    }

    /// Set the client to be used for fetching the data
    ///
    /// This is useful when you want to use a custom client with custom settings
    /// when using `Client::builder()`
    pub fn set_client(&mut self, client: Client) {
        self.client = client;
    }

    /// Set the response method to be used for fetching the response
    ///
    /// By default `self.response_method = ResponseMethod::Bytes`
    ///
    /// - `Bytes`: Fetch the full response using the `bytes` method
    /// - `Chunk`: Fetch the response in chunks using the `chunk` method
    /// - `BytesStream`: Fetch the response in a stream of bytes using the `bytes_stream` method
    ///
    /// If `response_method` is `Chunk` or `BytesStream`, then `notify_method` is set to `ProgressBar`
    /// and a `MultiProgress` instance is created
    pub fn set_response_method(&mut self, response_method: ResponseMethod) {
        self.response_method = response_method;
    }

    /// Set the notify method to be used for notifying the user
    /// By default `self.notify_method = NotifyMethod::Log`
    pub fn set_notify_method(&mut self, notify_method: NotifyMethod) {
        self.notify_method = notify_method;
        if notify_method == NotifyMethod::ProgressBar {
            assert!(
                (self.response_method == ResponseMethod::BytesStream)
                    || (self.response_method == ResponseMethod::Chunk)
            )
        }
    }
}

// Database Operations
impl<E: Entry + Clone + Send + Sync + 'static> Fetcher<E> {
    /// Removes the `db` directory (use with caution as this uses `tokio::fs::remove_dir_all`)
    pub async fn remove_db_dir(&self) -> Result<()> {
        tokio::fs::remove_dir_all(&self.db_path).await?;
        Ok(())
    }

    /// Remove the db and all its trees
    pub fn remove_db_trees(&self) -> Result<()> {
        let trees = self.db.tree_names();
        for tree in trees {
            self.db.drop_tree(tree)?;
        }
        Ok(())
    }

    /// Remove current tree
    pub fn remove_tree(&self) -> Result<()> {
        let tree = self.db.name();
        self.db.drop_tree(tree)?;
        Ok(())
    }

    /// Export the db to a vector of key value pairs and an iterator of values
    ///
    /// > Useful when needing to migrate the db from an older version to a newer version
    pub fn export(&self) -> Vec<(Vec<u8>, Vec<u8>, impl Iterator<Item = Vec<Vec<u8>>> + Sized)> {
        self.db.export()
    }

    /// Import the db from a vector of key value pairs and an iterator of values
    ///
    /// > Useful when needing to migrate the db from an older version to a newer version
    pub fn import(
        &self,
        export: Vec<(Vec<u8>, Vec<u8>, impl Iterator<Item = Vec<Vec<u8>>> + Sized)>,
    ) {
        self.db.import(export)
    }

    pub fn clear(&self) -> Result<()> {
        self.db.clear()?;
        Ok(())
    }
}

// Handles and Fetching Entries
impl<E: Entry + Clone + Send + Sync + 'static + for<'de> Deserialize<'de>> Fetcher<E> {
    async fn resp_bytes(&self, response: Response, name: String) -> Result<Bytes> {
        let len = response.content_length().unwrap_or(0);
        let style = ProgressStyle::default_spinner()
            .template("[{msg}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-");

        match &self.response_method {
            ResponseMethod::Bytes => {
                let bytes = response.bytes().await?;
                Ok(bytes)
            }
            ResponseMethod::BytesStream => {
                let mut stream = response.bytes_stream();
                let mut bytes = bytes::BytesMut::new();

                let pb = self.multi_pb.add(ProgressBar::new(len));
                pb.set_style(style.clone());
                pb.set_message(name.clone());
                let mut downloaded: u64 = 0;

                while let Some(item) = stream.next().await {
                    let b = item?;
                    downloaded += b.len() as u64;
                    bytes.extend_from_slice(&b);
                    pb.set_position(downloaded);
                }
                pb.finish();

                Ok(bytes.freeze())
            }
            ResponseMethod::Chunk => {
                let mut bytes = bytes::BytesMut::new();
                let mut response = response;

                let pb = self.multi_pb.add(ProgressBar::new(len));
                pb.set_style(style.clone());
                pb.set_message(name.clone());
                let mut downloaded: u64 = 0;
                while let Some(chunk) = response.chunk().await? {
                    downloaded += chunk.len() as u64;
                    bytes.extend_from_slice(&chunk);
                    pb.set_position(downloaded);
                }
                pb.finish();
                Ok(bytes.freeze())
            }
        }
    }

    /// Enables to fetch packages in a watching state from a config file
    /// The config file is watched for changes and the packages are fetched
    ///
    /// The fetching method is `Async` and the notification method is `log`
    pub async fn watching(&mut self) {
        info!("Watching {}", &self.config_path.display());
        if let Err(e) = self.watch().await {
            error!("Error: {:?}", e)
        }
    }

    async fn watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
        let (tx, rx) = channel(1);
        let watcher = RecommendedWatcher::new(
            move |res| {
                let tx = tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = tx.clone().send(res).await {
                        eprintln!("Error sending event: {}", e);
                    }
                });
            },
            NConfig::default(),
        )?;
        Ok((watcher, rx))
    }

    async fn watch(&mut self) -> notify::Result<()> {
        self.notify_method = NotifyMethod::Log;
        let (mut watcher, mut rx) = Self::watcher().await?;
        watcher.watch(&self.config_path, RecursiveMode::Recursive)?;

        while let Some(res) = rx.recv().await {
            match res {
                Ok(event) => self.handle_event(event).await.expect("Event handle error"),
                Err(e) => error!("Watch error: {:?}", e),
            }
        }
        Ok(())
    }

    async fn handle_event(&mut self, event: Event) -> anyhow::Result<()> {
        info!("Event: {:?}", event.kind);
        match event.kind {
            EventKind::Modify(_) => {
                self.config = Config::from_file(&self.config_path, self.config_type).await?;
                self.async_fetch().await?;
            }
            EventKind::Remove(_) => {
                info!("Removed {}", &self.config_path.display());
                info!("Clearing DB");
                self.db.clear().unwrap();
            }
            _ => debug!("Other event type"),
        }
        Ok(())
    }

    async fn handle_entry(&self, entry: E) -> Result<()> {
        let key = entry.key();
        let mut value = entry.value();

        if let Some(curr_val) = self.db.get(key.bytes())? {
            let cv_bytes = curr_val.to_vec();
            let cv = E::Value::from_ivec(curr_val);
            if !value.is_same(&cv) {
                if self.notify_method == NotifyMethod::Log {
                    key.log_caching();
                }
                let response = self.client.get(value.url()).send().await?;
                let bytes = self.resp_bytes(response, key.to_string()).await?;
                value.set_response(&bytes);
                let _ =
                    self.db
                        .compare_and_swap(key.bytes(), Some(cv_bytes), Some(value.bytes()))?;
            } else if self.notify_method == NotifyMethod::Log {
                key.log_cache();
            }
        } else {
            if self.notify_method == NotifyMethod::Log {
                key.log_caching();
            }
            let response = self.client.get(value.url()).send().await?;
            let bytes = self.resp_bytes(response, key.to_string()).await?;
            value.set_response(bytes.as_ref());
            let _ = self.db.insert(key.bytes(), value.bytes())?;
        }
        Ok(())
    }

    #[cfg(feature = "unstable")]
    fn handle_entry_sync(&self, entry: E) -> Result<()> {
        futures::executor::block_on(self.handle_entry(entry))?;
        Ok(())
    }

    /// Fetches and stores all results to the db
    pub async fn async_fetch(&mut self) -> Result<()> {
        let mut tasks = Vec::new();
        for entry in (*self.entries).clone() {
            let fetcher = self.clone();
            tasks.push(tokio::spawn(async move {
                fetcher.handle_entry(entry.clone()).await
            }));
        }

        join_all(tasks).await.into_iter().try_for_each(|x| x?)?;

        Ok(())
    }

    #[cfg(feature = "unstable")]
    /// Fetches and stores all results to the db synchronously and in parallel
    pub fn sync_fetch(&mut self) -> Result<()> {
        let entries = self.entries.clone();

        let results: Vec<Result<()>> = entries
            .par_iter()
            .map(|entry| self.handle_entry_sync(entry.clone()))
            .collect();

        results.into_iter().try_for_each(|x| x)?;

        Ok(())
    }

    pub async fn fetch(&mut self, method: FetchMethod) -> Result<()> {
        match method {
            FetchMethod::Async => self.async_fetch().await?,
            FetchMethod::Watch => self.watching().await,
            #[cfg(feature = "unstable")]
            FetchMethod::Sync => {
                println!("Please use sync_fetch() for synchronous operations.")
                // Honestly not sure how you would get here but here's a message I guess
            }
        }
        Ok(())
    }

    /// Returns all entries in the db as a vector of key-value pairs
    pub fn pairs<K: EntryKey, V: EntryValue>(&self) -> Result<Vec<(K, V)>> {
        self.db
            .iter()
            .map(|x| {
                let (key_iv, value_iv) = x.unwrap();
                let key = K::from_ivec(key_iv);
                Ok((key, V::from_ivec(value_iv)))
            })
            .collect()
    }

    /// Gets an entry from the db by key
    pub fn get<K: EntryKey, V: EntryValue>(&self, key: K) -> Result<Option<V>> {
        if let Some(value_iv) = self.db.get(key.bytes())? {
            let value = V::from_ivec(value_iv);
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Removes an entry from the db by key
    pub fn remove<K: EntryKey>(&self, key: K) -> Result<()> {
        self.db.remove(key.bytes())?;
        Ok(())
    }

    /// Updates an entry in the db by key and new value
    pub fn update<K: EntryKey, V: EntryValue>(&self, key: K, value: V) -> Result<()> {
        if let Some(curr_val) = self.db.get(key.bytes())? {
            let cv_bytes = curr_val.to_vec();
            let cv = V::from_ivec(curr_val);
            if !value.is_same(&cv) {
                let _ =
                    self.db
                        .compare_and_swap(key.bytes(), Some(cv_bytes), Some(value.bytes()))?;
            }
        }
        Ok(())
    }

    /// Writes all the fetched data to the specified directory
    pub async fn write_all(&self, dir: PathBuf) -> Result<()> {
        let total_entries = self.entries.len();
        let progress_bar = Arc::new(Mutex::new(ProgressBar::new(total_entries as u64)));
        progress_bar.lock().await.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {wide_msg}")
                .unwrap()
                .progress_chars("##-"),
        );

        let mut tasks = Vec::new();
        for entry in (*self.entries).clone() {
            let key = entry.key();
            let value_vec = self.db.get(key.bytes())?.unwrap().to_vec();
            let value: E::Value = E::Value::from_bytes(&value_vec);
            let resp = value.response();
            let file_name = Url::parse(&value.url())?
                .path_segments()
                .unwrap()
                .last()
                .unwrap()
                .to_string();
            let path = dir.join(&file_name);
            if !dir.exists() {
                create_dir(&dir).await?;
            }
            let bytes = resp.to_vec();
            let pb_clone = Arc::clone(&progress_bar);
            tasks.push(tokio::spawn(async move {
                pb_clone
                    .lock()
                    .await
                    .set_message(format!("Writing: {}", file_name));
                let result = tokio::fs::write(&path, bytes).await;
                pb_clone.lock().await.inc(1);
                result
            }));
        }

        let results = join_all(tasks).await;
        progress_bar
            .lock()
            .await
            .finish_with_message("All files written");

        results.into_iter().try_for_each(|x| x?)?;
        Ok(())
    }
}

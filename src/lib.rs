#![doc = include_str!("../README.md")]
#[macro_use]
extern crate log;

use anyhow::Result;
pub use bincode;
use bytes::Bytes;
use encryption::EncryptionMethod;
use futures::future::join_all;
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use kanal::bounded_async;
use notify::{Config as NConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use package::{Config, Mode};
pub use pretty_env_logger;
pub use quickfetch_traits as traits;
use quickfetch_traits::{Entry, EntryKey, EntryValue};
use reqwest::{Client, Response};
use serde::Deserialize;
use sled::Db;
use sled::IVec;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::create_dir;
use tokio::sync::mpsc::{channel, Receiver};
use tokio::sync::Semaphore;
use url::Url;
/// Provides different types of encryption methods that can be used
pub mod encryption;
/// Provides structures that can be used as a Key and Value for Fetcher
pub mod key_val;
/// Provides different types of packages that can be used
pub mod package;

/// Returns the path to the home directory with the sub directory appended
pub fn home_plus<P: AsRef<Path>>(sub_dir: P) -> PathBuf {
    dirs::home_dir().unwrap().join(sub_dir)
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
/// - `Concurrent`: Fetch the response concurrently using `tokio::spawn`
/// - `Channel`: Fetch the response using a bounded multi-producer single-consumer channel
#[derive(Debug, Copy, Clone)]
pub enum FetchMethod {
    Concurrent,
    Channel,
}

impl Default for FetchMethod {
    fn default() -> Self {
        Self::Concurrent
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
    entries: Arc<Vec<E>>,
    config_path: PathBuf,
    config: Config<E>,
    config_type: Mode,
    db: Db,
    db_path: PathBuf,
    client: Client,
    response_method: ResponseMethod,
    encryption_method: Option<EncryptionMethod>,
    notify_method: NotifyMethod,
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
            .brotli(true) // by default enable brotli compression
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
            encryption_method: None,
            notify_method: NotifyMethod::Log,
            multi_pb: Arc::new(MultiProgress::new()),
        })
    }

    /// Set the client to be used for fetching the data
    ///
    /// This is useful when you want to use a custom client with custom settings
    /// when using `Client::builder()`
    pub fn set_client(&mut self, client: Client) {
        self.client = client;
    }

    /// Set the encryption method to be used for encrypting and decrypting the response data
    /// By default `self.encryption_method = None`
    pub fn set_encryption_method(&mut self, encryption_method: EncryptionMethod) {
        self.encryption_method = Some(encryption_method)
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
        if response_method == ResponseMethod::Chunk
            || response_method == ResponseMethod::BytesStream
        {
            self.notify_method = NotifyMethod::ProgressBar;
        }
    }

    /// Set the notify method to be used for notifying the user
    /// By default `self.notify_method = NotifyMethod::Log`
    pub fn set_notify_method(&mut self, notify_method: NotifyMethod) {
        self.notify_method = notify_method;
        if notify_method == NotifyMethod::ProgressBar {}
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
        let style = ProgressStyle::default_bar()
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

                while let Some(item) = stream.next().await {
                    let b = item?;
                    bytes.extend_from_slice(&b);
                    pb.inc(b.len() as u64);
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
                while let Some(chunk) = response.chunk().await? {
                    bytes.extend_from_slice(&chunk);
                    pb.inc(chunk.len() as u64);
                }
                pb.finish();
                Ok(bytes.freeze())
            }
        }
    }

    /// Enables to fetch packages in a watching state from a config file
    /// The config file is watched for changes and the packages are fetched
    ///
    /// The fetching method is `concurrent` and the notification method is `log`
    pub async fn watching(&mut self) {
        println!("Watching {}", &self.config_path.display());
        if let Err(e) = self.watch().await {
            error!("Error: {:?}", e)
        }
    }

    async fn watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
        let (tx, rx) = channel(1);

        let watcher = RecommendedWatcher::new(
            move |res| {
                futures::executor::block_on(async {
                    tx.send(res).await.unwrap();
                })
            },
            NConfig::default(),
        )?;

        Ok((watcher, rx))
    }

    async fn watch(&mut self) -> notify::Result<()> {
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
                self.concurrent_fetch().await?;
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

    fn encrypt_bytes(&self, bytes: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        if let Some(encryption_method) = &self.encryption_method {
            let bytes = encryption_method.encrypt(bytes.as_ref(), key)?;
            Ok(bytes)
        } else {
            Ok(bytes.to_vec())
        }
    }

    async fn handle_entry_channel(&self, entry: E, bytes: Option<Bytes>) -> Result<()> {
        let key = entry.key();
        let mut value = entry.value();

        let key_bytes = key.bytes();

        // key doesn't exist in the db
        if let Some(bytes) = bytes {
            if self.notify_method == NotifyMethod::Log {
                key.log_caching();
            }
            let bytes = self.encrypt_bytes(bytes.as_ref(), key_bytes.as_ref())?;
            value.set_response(bytes.as_ref());
            let _ = self.db.insert(key.bytes(), value.bytes())?;
        } else if let Some(curr_val) = self.db.get(&key_bytes)? {
            let cv_bytes = curr_val.to_vec();
            let cv = E::Value::from_ivec(curr_val);
            if !value.is_same(&cv) {
                if self.notify_method == NotifyMethod::Log {
                    key.log_caching();
                }
                let response = self.client.get(&value.url()).send().await?;
                let bytes = self.resp_bytes(response, key.to_string()).await?;
                value.set_response(&self.encrypt_bytes(bytes.as_ref(), &key.bytes()).unwrap());
                let _ =
                    self.db
                        .compare_and_swap(key.bytes(), Some(cv_bytes), Some(value.bytes()))?;
            } else if self.notify_method == NotifyMethod::Log {
                key.log_cache();
            }
        }

        Ok(())
    }

    /// Fetches and stores all results to the db using a bounded multi-producer single-consumer channel
    /// Benefits of multiple entries since we use `recv_many` to receive all the entries at once
    pub async fn channel_fetch(&self) -> Result<()> {
        let len = self.entries.len();
        let (tx, rx) = bounded_async(len);
        let self_clone = self.clone();
        let semaphore = Arc::new(Semaphore::new(len)); // Adjust the concurrency limit as needed

        // Use a stream to process entries concurrently
        let entries = self_clone.entries.clone();
        let entries_stream = futures::stream::iter(<Vec<E> as Clone>::clone(&entries).into_iter())
            .for_each_concurrent(None, move |entry| {
                let tx = tx.clone();
                let self_clone = self_clone.clone();
                let semaphore = semaphore.clone();

                async move {
                    let _permit = semaphore.acquire().await.unwrap(); // Acquire a semaphore permit

                    let mut bytes: Option<Bytes> = None;
                    if !self_clone.db.contains_key(entry.key().bytes()).unwrap() {
                        let response = self_clone
                            .client
                            .get(entry.value().url())
                            .send()
                            .await
                            .unwrap();
                        bytes = Some(
                            self_clone
                                .resp_bytes(response, entry.key().to_string())
                                .await
                                .unwrap(),
                        );
                    }
                    tx.send((entry.clone(), bytes)).await.unwrap();
                }
            });

        // Await the completion of the stream
        tokio::spawn(entries_stream).await?;

        rx.stream()
            .for_each(|(entry, bytes)| async {
                self.handle_entry_channel(entry, bytes).await.unwrap();
            })
            .await;

        Ok(())
    }

    async fn handle_entry_conc(&self, entry: E) -> Result<()> {
        let key = entry.key();
        let mut value = entry.value();

        if let Some(curr_val) = self.db.get(key.bytes())? {
            let cv_bytes = curr_val.to_vec();
            let cv = E::Value::from_ivec(curr_val);
            if !value.is_same(&cv) {
                if self.notify_method == NotifyMethod::Log {
                    key.log_caching();
                }
                let response = self.client.get(&value.url()).send().await?;
                let bytes = self.resp_bytes(response, key.to_string()).await?;
                value.set_response(&self.encrypt_bytes(bytes.as_ref(), &key.bytes()).unwrap());
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
            let response = self.client.get(&value.url()).send().await?;
            let bytes = self.resp_bytes(response, key.to_string()).await?;
            value.set_response(&self.encrypt_bytes(bytes.as_ref(), &key.bytes()).unwrap());
            let _ = self.db.insert(key.bytes(), value.bytes())?;
        }
        Ok(())
    }

    /// Fetches and stores all results to the db
    pub async fn concurrent_fetch(&mut self) -> Result<()> {
        let mut tasks = Vec::new();
        for entry in (*self.entries).clone() {
            let fetcher = self.clone();
            tasks.push(tokio::spawn(async move {
                fetcher.handle_entry_conc(entry.clone()).await
            }));
        }

        join_all(tasks).await.into_iter().try_for_each(|x| x?)?;

        Ok(())
    }

    pub async fn fetch(&mut self, method: FetchMethod) -> Result<()> {
        match method {
            FetchMethod::Concurrent => self.concurrent_fetch().await?,
            FetchMethod::Channel => self.channel_fetch().await?,
        }
        Ok(())
    }

    fn dec_or_clone(
        encryption_method: Option<EncryptionMethod>,
        value: &[u8],
        key: &[u8],
    ) -> Result<Vec<u8>> {
        if let Some(encryption_method) = encryption_method {
            encryption_method.decrypt(value, key)
        } else {
            Ok(value.to_vec())
        }
    }

    /// Fetches and stores all results from the db
    pub fn pairs<K: EntryKey, V: EntryValue>(&self) -> Result<Vec<(K, V)>> {
        self.db
            .iter()
            .map(|x| {
                let (key_iv, value_iv) = x.unwrap();
                let key = K::from_ivec(key_iv);
                let bytes =
                    Self::dec_or_clone(self.encryption_method, value_iv.as_ref(), &key.bytes())?;
                Ok((key, V::from_ivec(IVec::from(bytes))))
            })
            .collect()
    }

    /// Gets an entry from the db by key
    pub fn get<K: EntryKey, V: EntryValue>(&self, key: K) -> Result<Option<V>> {
        if let Some(value_iv) = self.db.get(key.bytes())? {
            let bytes =
                Self::dec_or_clone(self.encryption_method, value_iv.as_ref(), &key.bytes())?;
            Ok(Some(V::from_ivec(IVec::from(bytes))))
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
        let mut tasks = Vec::new();

        for entry in (*self.entries).clone() {
            let key = entry.key();
            let ivec = self.db.get(key.bytes())?.unwrap();
            let value = E::Value::from_ivec(ivec);
            let resp = value.response();
            let file_name = Url::parse(&value.url())?
                .path_segments()
                .unwrap()
                .last()
                .unwrap()
                .to_string();
            let path = dir.join(file_name);
            if !dir.exists() {
                create_dir(&dir).await?;
            }

            let bytes = Self::dec_or_clone(self.encryption_method, resp.deref(), &key.bytes())?;
            tasks.push(tokio::spawn(
                async move { tokio::fs::write(path, bytes).await },
            ))
        }

        join_all(tasks).await.into_iter().try_for_each(|x| x?)?;
        Ok(())
    }
}

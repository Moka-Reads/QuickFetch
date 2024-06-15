//! # QuickFetch
//!
//! Developed by Mustafif Khan | MoKa Reads 2024
//!
//! **This library is under the MIT License**
//!
//! This library is built to handle multiple requests within a `Client` (`reqwest` client which will handle it all under a Client Pool)
//! , cache the response results, and handle these in parallel and asynchronously.
//!
//! The goal is to be a one-stop shop for handling local package manager development to handle multiple
//! packages with a local cache to easily update, get and remove the different responses.
//!
//! ## Example
//! ```rust
//! use quickfetch::{Fetcher,package::{Package, Config}, FetchMethod};
//! use quickfetch_traits::Entry;
//! use quickfetch::home_plus;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!    quickfetch::pretty_env_logger::init();
//!    let config: Config<Package> = Config::from_toml_file("examples/pkgs.toml").await?;
//!    // store db in $HOME/.quickfetch
//!    let mut fetcher: Fetcher<Package> = Fetcher::new(config.packages(), home_plus(".quickfetch"))?;
//!    fetcher.fetch(FetchMethod::Channel).await?;
//!    // write the packages to $HOME/pkgs
//!    fetcher.write_all(home_plus("pkgs")).await?;
//!    Ok(())
//! }
//! ```

#[macro_use]
extern crate log;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use futures::future::join_all;
use futures::StreamExt;

use quickfetch_traits::Entry;
use reqwest::{Client, Response};
use sled::{Batch, Db, IVec};
use tokio::fs::create_dir;
use tokio::sync::mpsc::channel;
use tokio::sync::RwLock;
use url::Url;

pub use pretty_env_logger;
pub use bincode;
pub use quickfetch_derive as macros;
pub use quickfetch_traits as traits;

use encryption::EncryptionMethod;

/// Provides different types of encryption methods that can be used
pub mod encryption;
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
#[derive(Debug, Copy, Clone)]
pub enum ResponseMethod {
    Bytes,
    Chunk,
    BytesStream,
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
        Self::Channel
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
#[derive(Debug, Clone)]
pub struct Fetcher<E: Entry> {
    pub entries: Vec<E>,
    db: Db,
    db_path: PathBuf,
    client: Client,
    response_method: ResponseMethod,
    encryption_method: Option<EncryptionMethod>,
}

#[derive(Debug, Clone, Copy)]
pub enum SelectOp {
    Update,
    Delete,
    Get,
}

impl<E: Entry + Clone + Send + Sync + 'static> Fetcher<E> {
    /// Create a new `Fetcher` instance with list of urls and db path
    pub fn new<P: AsRef<Path>>(entries: &[E], db_path: P) -> Result<Self> {
        let client = Client::builder()
            .brotli(true) // by default enable brotli compression
            .build()?;

        Ok(Self {
            entries: entries.to_vec(),
            db: sled::open(&db_path)?,
            db_path: PathBuf::from(db_path.as_ref()),
            client,
            response_method: ResponseMethod::default(),
            encryption_method: None,
        })
    }

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

    /// Set the client to be used for fetching the data
    ///
    /// This is useful when you want to use a custom client with custom settings
    /// when using `Client::builder()`
    pub fn set_client(&mut self, client: Client) {
        self.client = client;
    }

    /// Set the encryption method to be used for encrypting and decrypting the response data
    pub fn set_encryption_method(&mut self, encryption_method: EncryptionMethod) {
        self.encryption_method = Some(encryption_method)
    }

    /// Set the response method to be used for fetching the response
    pub fn set_response_method(&mut self, response_method: ResponseMethod) {
        self.response_method = response_method
    }

    async fn resp_bytes(&self, response: Response) -> Result<bytes::Bytes> {
        match &self.response_method {
            ResponseMethod::Bytes => {
                let bytes = response.bytes().await?;
                Ok(bytes)
            }
            ResponseMethod::BytesStream => {
                let mut stream = response.bytes_stream();
                let mut bytes = bytes::BytesMut::new();
                while let Some(item) = stream.next().await {
                    let b = item?;
                    bytes.extend_from_slice(&b)
                }

                Ok(bytes.freeze())
            }
            ResponseMethod::Chunk => {
                let mut bytes = bytes::BytesMut::new();
                let mut response = response;
                while let Some(chunk) = response.chunk().await? {
                    bytes.extend_from_slice(&chunk)
                }
                Ok(bytes.freeze())
            }
        }
    }

    /// Fetches and stores all results to the db
    pub async fn concurrent_fetch(&mut self) -> Result<()> {
        let mut tasks = Vec::new();
        let fetcher = Arc::new(RwLock::new(self.clone()));

        for entry in self.entries.clone() {
            let fetcher = fetcher.clone();
            tasks.push(tokio::spawn(async move {
                let mut fetcher = fetcher.write().await;
                fetcher.handle_entry_conc(entry).await
            }));
        }

        let j = join_all(tasks)
            .await
            .into_iter()
            .map(|x| Ok(x??))
            .collect::<Result<()>>()?;

        Ok(j)
    }

    async fn handle_entry_channel(&mut self, entry: E, bytes: Bytes) -> Result<()> {
        if self.db.get(&entry.entry_bytes())?.is_some() {
            entry.log_cache()
        } else if let Some(old_key) = entry.is_modified(self.db.iter().keys()) {
            self.handle_modified_entry(entry, bytes, old_key).await?;
        } else {
            self.handle_new_entry(entry, bytes).await?;
        }
        Ok(())
    }

    async fn handle_entry_conc(&mut self, entry: E) -> Result<()> {
        if self.db.get(&entry.entry_bytes())?.is_some() {
            entry.log_cache();
        } else if let Some(old_key) = entry.is_modified(self.db.iter().keys()) {
            let response = self.client.get(&entry.url()).send().await?;
            let bytes = self.resp_bytes(response).await?;
            self.handle_modified_entry(entry, bytes, old_key).await?;
        } else {
            let response = self.client.get(&entry.url()).send().await?;
            let bytes = self.resp_bytes(response).await?;
            self.handle_new_entry(entry, bytes).await?;
        }
        Ok(())
    }

    async fn handle_modified_entry(&mut self, entry: E, bytes: Bytes, old_key: IVec) -> Result<()> {
        let mut batch = Batch::default();
        batch.remove(old_key);
        entry.log_caching();

        // Encrypt the bytes before inserting into the db
        let bytes = self.encrypt_bytes(bytes, &entry.entry_bytes())?;
        batch.insert(entry.entry_bytes(), bytes.as_ref());
        self.db.apply_batch(batch)?;
        Ok(())
    }

    async fn handle_new_entry(&mut self, entry: E, bytes: Bytes) -> Result<()> {
        entry.log_caching();

        // Encrypt the bytes before inserting into the db
        let bytes = self.encrypt_bytes(bytes, &entry.entry_bytes())?;
        let _ = self.db.insert(entry.entry_bytes(), bytes.as_ref())?;
        Ok(())
    }

    fn encrypt_bytes(&self, bytes: Bytes, key: &[u8]) -> Result<Bytes> {
        if let Some(encryption_method) = &self.encryption_method {
            let bytes = encryption_method.encrypt(bytes.as_ref(), key)?;
            Ok(Bytes::from(bytes))
        } else {
            Ok(bytes)
        }
    }

    /// Fetches and stores all results to the db using a bounded multi-producer single-consumer channel
    /// Benefits of multiple entries since we use `recv_many` to receive all the entries at once
    pub async fn channel_fetch(&mut self) -> Result<()> {
        let (tx, mut rx) = channel(self.entries.len());

        let self_clone = self.clone();
        let handle = tokio::spawn(async move {
            for entry in self_clone.entries.clone() {
                let tx = tx.clone();

                let response = self_clone.client.get(&entry.url()).send().await.unwrap();
                let bytes = self_clone.resp_bytes(response).await.unwrap();
                tx.send((entry, bytes)).await.unwrap();
            }
        });

        // Await the completion of the spawned task
        handle.await?;

        let mut entries = Vec::new();
        _ = rx.recv_many(&mut entries, self.entries.len()).await;
        for (entry, bytes) in entries.drain(..) {
            self.handle_entry_channel(entry, bytes).await?;
        }

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
    pub fn pairs(&self) -> Result<Vec<(E, Vec<u8>)>> {
        self.db
            .iter()
            .map(|x| {
                let (key, value) = x.unwrap();
                let bytes = Self::dec_or_clone(
                    self.encryption_method.clone(),
                    value.as_ref(),
                    key.as_ref(),
                )?;
                Ok((E::from_ivec(key), bytes))
            })
            .collect()
    }

    /// Selects the entry based on the key and operation
    ///
    /// - `Update`: Update the entry in the db (if successful returns `Ok(None)`)
    /// - `Delete`: Delete the entry in the db (if successful returns `Ok(None)`)
    /// - `Get`: Get the entry in the db (if successful returns `Ok(Some(Vec<u8>))`)
    pub async fn select(&self, key: E, op: SelectOp) -> Result<Option<Vec<u8>>> {
        match op {
            SelectOp::Update => {
                if let Some(old_key) = key.is_modified(self.db.iter().keys()) {
                    let mut batch = Batch::default();
                    batch.remove(old_key);
                    let response = self.client.get(&key.url()).send().await?;
                    let bytes = self.resp_bytes(response).await?;

                    // Encrypt the bytes before inserting into the db
                    if let Some(encryption_method) = &self.encryption_method {
                        let bytes =
                            encryption_method.encrypt(bytes.as_ref(), &key.entry_bytes())?;
                        batch.insert(key.entry_bytes(), bytes.as_slice());
                    } else {
                        batch.insert(key.entry_bytes(), bytes.as_ref());
                    }
                    self.db.apply_batch(batch)?;
                }
                Ok(None)
            }

            SelectOp::Delete => {
                let key = key.entry_bytes();
                let _ = self.db.remove(&key)?;
                Ok(None)
            }
            SelectOp::Get => {
                let key = key.entry_bytes();
                let resp = self.db.get(&key)?;
                if let Some(ivec) = resp {
                    let bytes = Self::dec_or_clone(
                        self.encryption_method.clone(),
                        ivec.as_ref(),
                        key.as_ref(),
                    )?;
                    Ok(Some(bytes))
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Writes all the fetched data to the specified directory
    pub async fn write_all(&self, dir: PathBuf) -> Result<()> {
        let mut tasks = Vec::new();

        for entry in self.entries.clone() {
            let key = entry.url();
            let resp = self.db.get(&key)?;
            let file_name = Url::parse(&key)?
                .path_segments()
                .unwrap()
                .last()
                .unwrap()
                .to_string();
            let path = dir.join(file_name);
            if !dir.exists() {
                create_dir(&dir).await?;
            }
            if let Some(ivec) = resp {
                let bytes = Self::dec_or_clone(
                    self.encryption_method.clone(),
                    ivec.as_ref(),
                    key.as_bytes(),
                )?;
                tasks.push(tokio::spawn(async move { tokio::fs::write(path, bytes) }))
            }
        }

        let res = join_all(tasks).await;
        for r in res {
            r?.await?;
        }
        Ok(())
    }
}

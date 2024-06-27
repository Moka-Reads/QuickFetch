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

use sled::IVec;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
pub use bincode;
use bytes::Bytes;
use encryption::EncryptionMethod;
use futures::future::join_all;
use futures::StreamExt;
use kanal::bounded_async as channel;
pub use pretty_env_logger;
pub use quickfetch_traits as traits;
use quickfetch_traits::{Entry, EntryKey, EntryValue};
use reqwest::{Client, Response};
use sled::Db;
use tokio::fs::create_dir;
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

// Constructor and Setup Methods
impl<E: Entry + Clone + Send + Sync + 'static> Fetcher<E> {
    /// Create a new `Fetcher` instance with list of urls and db path
    pub fn new<P: AsRef<Path>>(entries: &[E], db_path: P) -> Result<Self> {
        let client = Client::builder()
            .brotli(true) // by default enable brotli compression
            .build()?;

        Ok(Self {
            entries: Arc::new(entries.to_vec()),
            db: sled::open(&db_path)?,
            db_path: PathBuf::from(db_path.as_ref()),
            client,
            response_method: ResponseMethod::default(),
            encryption_method: None,
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
    pub fn set_response_method(&mut self, response_method: ResponseMethod) {
        self.response_method = response_method
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
}

// Handles and Fetching Entries
impl<E: Entry + Clone + Send + Sync + 'static> Fetcher<E> {
    async fn resp_bytes(&self, response: Response) -> Result<Bytes> {
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

    // async fn handle_modified_entry(&mut self, entry: E, bytes: Bytes, old_key: IVec) -> Result<()> {
    //     let mut batch = Batch::default();
    //     batch.remove(old_key);
    //     entry.log_caching();

    //     // Encrypt the bytes before inserting into the db
    //     let bytes = self.encrypt_bytes(bytes, &entry.entry_bytes())?;
    //     batch.insert(entry.entry_bytes(), bytes.as_ref());
    //     self.db.apply_batch(batch)?;
    //     Ok(())
    // }

    // async fn handle_new_entry(&mut self, entry: E, bytes: Bytes) -> Result<()> {
    //     entry.log_caching();

    //     // Encrypt the bytes before inserting into the db
    //     let bytes = self.encrypt_bytes(bytes, &entry.entry_bytes())?;
    //     let _ = self.db.insert(entry.entry_bytes(), bytes.as_ref())?;
    //     Ok(())
    // }

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
            key.log_caching();
            let bytes = self.encrypt_bytes(bytes.as_ref(), key_bytes.as_ref())?;
            value.set_response(bytes.as_ref());
            let _ = self.db.insert(key.bytes(), value.bytes())?;
        } else if let Some(curr_val) = self.db.get(&key_bytes)? {
            let cv_bytes = curr_val.to_vec();
            let cv = E::Value::from_ivec(curr_val);
            if !value.is_same(&cv) {
                key.log_caching();
                let response = self.client.get(&value.url()).send().await?;
                let bytes = self.resp_bytes(response).await?;
                value.set_response(&self.encrypt_bytes(bytes.as_ref(), &key.bytes()).unwrap());
                let _ =
                    self.db
                        .compare_and_swap(key.bytes(), Some(cv_bytes), Some(value.bytes()))?;
            } else {
                key.log_cache();
            }
        }

        Ok(())
    }

    /// Fetches and stores all results to the db using a bounded multi-producer single-consumer channel
    /// Benefits of multiple entries since we use `recv_many` to receive all the entries at once
    pub async fn channel_fetch(&self) -> Result<()> {
        let len = self.entries.len();
        let (tx, rx) = channel(len);
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
                        bytes = Some(self_clone.resp_bytes(response).await.unwrap());
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
                key.log_caching();
                let response = self.client.get(&value.url()).send().await?;
                let bytes = self.resp_bytes(response).await?;
                value.set_response(&self.encrypt_bytes(bytes.as_ref(), &key.bytes()).unwrap());
                let _ =
                    self.db
                        .compare_and_swap(key.bytes(), Some(cv_bytes), Some(value.bytes()))?;
            } else {
                key.log_cache();
            }
        } else {
            key.log_caching();
            let response = self.client.get(&value.url()).send().await?;
            let bytes = self.resp_bytes(response).await?;
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

    /// Selects the entry based on the key and operation
    ///
    /// - `Update`: Update the entry in the db (if successful returns `Ok(None)`)
    /// - `Delete`: Delete the entry in the db (if successful returns `Ok(None)`)
    /// - `Get`: Get the entry in the db (if successful returns `Ok(Some(Vec<u8>))`)
    pub async fn select(&self, key: E, op: SelectOp) -> Result<Option<Vec<u8>>> {
        match op {
            SelectOp::Update => {
                // if let Some(old_key) = key.is_modified(self.db.iter().keys()) {
                //     let mut batch = Batch::default();
                //     batch.remove(old_key);
                //     let response = self.client.get(&key.url()).send().await?;
                //     let bytes = self.resp_bytes(response).await?;

                //     // Encrypt the bytes before inserting into the db
                //     if let Some(encryption_method) = &self.encryption_method {
                //         let bytes =
                //             encryption_method.encrypt(bytes.as_ref(), &key.entry_bytes())?;
                //         batch.insert(key.entry_bytes(), bytes.as_slice());
                //     } else {
                //         batch.insert(key.entry_bytes(), bytes.as_ref());
                //     }
                //     self.db.apply_batch(batch)?;
                // }
                Ok(None)
            }

            SelectOp::Delete => {
                // let key = key.entry_bytes();
                // let _ = self.db.remove(key)?;
                Ok(None)
            }
            SelectOp::Get => {
                // let key = key.entry_bytes();
                // let resp = self.db.get(&key)?;
                // if let Some(ivec) = resp {
                //     let bytes =
                //         Self::dec_or_clone(self.encryption_method, ivec.as_ref(), key.as_ref())?;
                //     Ok(Some(bytes))
                // } else {
                //     Ok(None)
                // }
                Ok(None)
            }
        }
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

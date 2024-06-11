//! # QuickFetch
//!
//! Developed by Mustafif Khan | MoKa Reads 2024
//!
//! > This library is under the MIT License
//!
//! This library is built to handle multiple requests within a `Client` (`reqwest` client which will handle it all under a Client Pool)
//! , cache the response results, and handle these in parallel and asynchronously.
//!
//! The goal is to be a one-stop shop for handling local package manager development to handle multiple
//! packages with a local cache to easily update, get and remove the different responses.

/// Provides different types of encryption methods that can be used
pub mod encrption;
/// Provides different types of packages that can be used
pub mod package;

#[macro_use]
extern crate log;
pub use pretty_env_logger;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use encrption::EncryptionMethod;
use futures::future::join_all;
use futures::StreamExt;
use reqwest::{Client, Response};
use sled::{Batch, Db, IVec};
use tokio::fs::create_dir;
use tokio::sync::RwLock;
use url::Url;

/// Returns the path to the home directory with the sub directory appended
pub fn home_plus<P: AsRef<Path>>(sub_dir: P) -> PathBuf {
    dirs::home_dir().unwrap().join(sub_dir)
}

/// Entry trait that will be used to be able to fetch and cache data as the Key
pub trait Entry {
    /// Check if the entry is modified inside of the db's key iterator and return the old key
    fn is_modified(
        &self,
        keys_iter: impl DoubleEndedIterator<Item = Result<IVec, sled::Error>>,
    ) -> Option<IVec>;
    /// Return the url of the entry to send the `GET` request to
    fn url(&self) -> String;
    /// Return the entry serialized as bytes to be used as the key in the db
    fn entry_bytes(&self) -> Vec<u8>;
    /// Log that the entry is cached
    fn log_cache(&self);
    /// Log that the entry is being cached
    fn log_caching(&self);

    fn from_ivec(value: IVec) -> Self where Self:Sized;
}

impl Entry for String {
    fn is_modified(
        &self,
        _keys_iter: impl DoubleEndedIterator<Item = Result<IVec, sled::Error>>,
    ) -> Option<IVec> {
        None
    }

    fn url(&self) -> String {
        self.to_string()
    }

    fn entry_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    fn log_cache(&self) {
        info!("{} (cached)", self.url())
    }

    fn log_caching(&self) {
        info!("{} caching", self.url())
    }

    fn from_ivec(value: IVec) -> Self where Self: Sized {
        String::from_utf8(value.to_vec()).unwrap()
    }
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

impl<E: Entry + Clone + Send + Sync + 'static> Fetcher<E> {
    /// Create a new `Fetcher` instance with list of urls and cache db name
    pub fn new(entries: &Vec<E>, db_path: &str) -> Result<Self> {
        let client = Client::builder()
            .brotli(true) // by default enable brotli compression
            .build()?;

        Ok(Self {
            entries: entries.to_owned(),
            db: sled::open(db_path)?,
            db_path: PathBuf::from(db_path),
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

    async fn fetch_entry(&mut self, entry: E) -> Result<()> {
        if self.db.get(&entry.entry_bytes())?.is_some() {
            entry.log_cache()
        } else if let Some(old_key) = entry.is_modified(self.db.iter().keys()) {
            let mut batch = Batch::default();
            batch.remove(old_key);
            let response = self.client.get(&entry.url()).send().await?;
            let bytes = self.resp_bytes(response).await?;
            entry.log_caching();

            // Encrypt the bytes before inserting into the db
            if let Some(encryption_method) = &self.encryption_method {
                let bytes = encryption_method.encrypt(bytes.as_ref(), &entry.entry_bytes())?;
                batch.insert(entry.entry_bytes(), bytes.as_slice());
            } else {
                batch.insert(entry.entry_bytes(), bytes.as_ref());
            }
            self.db.apply_batch(batch)?;
        } else {
            let response = self.client.get(&entry.url()).send().await?;
            let bytes = self.resp_bytes(response).await?;
            entry.log_caching();

            // Encrypt the bytes before inserting into the db
            if let Some(encryption_method) = &self.encryption_method {
                let bytes = encryption_method.encrypt(bytes.as_ref(), &entry.entry_bytes())?;
                let _ = self.db.insert(entry.entry_bytes(), bytes.as_slice())?;
            } else {
                let _ = self.db.insert(entry.entry_bytes(), bytes.as_ref())?;
            }
        }
        Ok(())
    }

    /// Fetches and stores all results to cache
    pub async fn fetch(&mut self) -> Result<()> {
        let mut tasks = Vec::new();
        let fetcher = Arc::new(RwLock::new(self.clone()));

        for entry in self.entries.clone() {
            let fetcher = fetcher.clone();
            tasks.push(tokio::spawn(async move {
                let mut fetcher = fetcher.write().await;
                fetcher.fetch_entry(entry).await
            }));
        }

        let j = join_all(tasks)
            .await
            .into_iter()
            .map(|x| Ok(x??))
            .collect::<Result<()>>()?;

        Ok(j)
    }

    /// Fetches and stores all results from the db
    pub fn pairs(&self) -> Result<Vec<(E, Vec<u8>)>> {
        self.db
            .iter()
            .map(|x| {
                let (key, value) = x.unwrap();
                let bytes = if let Some(encryption_method) = &self.encryption_method {
                    encryption_method.decrypt(value.as_ref(), key.as_ref())?
                } else {
                    value.to_vec()
                };
                Ok((E::from_ivec(key), bytes))
            })
            .collect()
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
                let bytes = if let Some(encryption_method) = &self.encryption_method {
                    encryption_method.decrypt(ivec.as_ref(), &entry.entry_bytes())?
                } else {
                    ivec.to_vec()
                };

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

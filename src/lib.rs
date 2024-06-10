pub mod package;

#[macro_use]
extern crate log;
pub use pretty_env_logger;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use futures::future::join_all;
use futures::StreamExt;
use reqwest::{Client, Response};
use sled::{Batch, Db, IVec};
use tokio::fs::create_dir;
use tokio::sync::RwLock;
use url::Url;

pub fn home_plus<P: AsRef<Path>>(sub_dir: P) -> PathBuf {
    dirs::home_dir().unwrap().join(sub_dir)
}

/// Entry trait that will be used to check if it has been modified
/// so `db` can compare and swap, and also return the `url`.
pub trait Entry {
    fn is_modified(&self, keys_iter: impl DoubleEndedIterator<Item=Result<IVec, sled::Error>>) -> Option<IVec>;
    fn url(&self) -> String;
    fn entry_bytes(&self) -> Vec<u8>;
    fn log_cache(&self);
    fn log_caching(&self);
}

impl Entry for String {
    fn is_modified(&self, _keys_iter: impl DoubleEndedIterator<Item=Result<IVec, sled::Error>>) -> Option<IVec> {
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
}

#[derive(Debug, Copy, Clone)]
pub enum ResponseMethod{
    Bytes,
    Chunk,
    BytesStream
}

impl Default for ResponseMethod{
    fn default() -> Self {
        Self::Bytes
    }
}

#[derive(Debug, Clone)]
pub struct Fetcher<E: Entry> {
    pub entries: Vec<E>,
    db: Db,
    client: Client,
    response_method: ResponseMethod,
}

impl<E: Entry + Clone + Send + Sync + 'static> Fetcher<E> {
    /// Create a new `Fetcher` instance with list of urls and cache db name
    pub fn new(entries: &Vec<E>, name: &str) -> Result<Self> {
        let client = Client::builder()
            .brotli(true) // by default enable brotli compression
            .build()?;

        Ok(Self {
            entries: entries.to_owned(),
            db: sled::open(name)?,
            client,
            response_method: ResponseMethod::default()
        })
    }

    pub fn set_client(&mut self, client: Client) {
        self.client = client;
    }

    pub fn set_response_method(&mut self, response_method: ResponseMethod){
        self.response_method = response_method
    }

    async fn resp_bytes(&self, response: Response) -> Result<bytes::Bytes>{
        match &self.response_method{
            ResponseMethod::Bytes => {
                let bytes = response.bytes().await?;
                Ok(bytes)
            },
            ResponseMethod::BytesStream => {
                let mut stream = response.bytes_stream();
                let mut bytes = bytes::BytesMut::new();
                while let Some(item) = stream.next().await{
                    let b = item?;
                    bytes.extend_from_slice(&b)
                }

                Ok(bytes.freeze())
            },
            ResponseMethod::Chunk => {
                let mut bytes = bytes::BytesMut::new();
                let mut response = response;
                while let Some(chunk) = response.chunk().await?{
                    bytes.extend_from_slice(&chunk)
                }
                Ok(bytes.freeze())
            }
        }
    }

    /// TODO!:Allow chunk and byte_stream methods
    async fn fetch_entry(&mut self, entry: E) -> Result<()> {
        if self.db.get(&entry.entry_bytes())?.is_some() {
            entry.log_cache()
        } else if let Some(old_key) = entry.is_modified(self.db.iter().keys()){
           let mut batch = Batch::default();
            batch.remove(old_key);
            let response = self.client.get(&entry.url()).send().await?;
            let bytes =  self.resp_bytes(response).await?;
            entry.log_caching();
            batch.insert(entry.entry_bytes(), bytes.as_ref());
            self.db.apply_batch(batch)?;
        }
        else {
            let response = self.client.get(&entry.url()).send().await?;
            let bytes = self.resp_bytes(response).await?;
            entry.log_caching();
            let _ = self.db.insert(entry.entry_bytes(), bytes.as_ref())?;
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
            .map(|x| x.unwrap().unwrap())
            .collect();

        Ok(j)
    }

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
                let bytes = ivec.to_vec();
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

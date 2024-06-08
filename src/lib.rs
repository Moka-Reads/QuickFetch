use std::collections::binary_heap::Iter;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use futures::future::join_all;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use reqwest::Client;
use sled::Db;
use tokio::fs::create_dir;
use tokio::sync::RwLock;
use url::Url;

/// Entry trait that will be used to check if it has been modified
/// so `db` can compare and swap, and also return the `url`.
pub trait Entry{
    fn is_modified(&self, other: &Self) -> bool;
    fn url(&self) -> String;
    fn entry_bytes(&self) -> &[u8];
}

impl Entry for String{
    fn is_modified(&self, _other: &String) -> bool {
        false // urls aren't modified
    }

    fn url(&self) -> String {
        self.to_string()
    }

    fn entry_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[derive(Debug, Clone)]
pub struct Fetcher<E: Entry> {
    pub entries: Vec<E>,
    db: Db,
    client: Client,
}

#[derive(Debug, Clone)]
pub struct EntryIterator<E: Entry>{
    inner: std::vec::IntoIter<E>
}

impl <E: Entry> Iterator for EntryIterator<E>{
    type Item = E;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<E: Entry> IntoIterator for Fetcher<E> {
    type Item = E;
    type IntoIter = EntryIterator<E>;

    fn into_iter(self) -> Self::IntoIter {
        EntryIterator {
            inner: self.entries.into_iter()
        }
    }
}

impl<E: Entry + Clone + Send + Sync + 'static + Iterator> Fetcher<E> {
    /// Create a new `Fetcher` instance with list of urls and cache db name
    pub fn new(urls: &Vec<E>, name: &str) -> Result<Self> {
        let client = Client::builder()
            .brotli(true) // by default enable brotli compression
            .build()?;

        Ok(Self {
            entries: urls.to_owned(),
            db: sled::open(name)?,
            client,
        })
    }

    pub fn iter(&self) -> EntryIterator<E>{
        EntryIterator{
            inner: self.entries.clone().into_iter()
        }
    }

    pub fn set_client(&mut self, client: Client) {
        self.client = client;
    }

    // TODO: switch from `println!` to logging `info`
    async fn fetch_entry(&mut self, entry: E) -> Result<()> {
        if self.db.get(&entry.url())?.is_some() {
            // add some logic to compare and swap
            println!("{} (cached)", entry.url());
        } else {
            let response = self.client.get(&entry.url()).send().await?;
            let bytes = response.bytes().await?;
            println!("{} caching", entry.url());
            let _ = self.db.insert(entry.entry_bytes(), bytes.as_ref())?;
        }
        Ok(())
    }

    /// Fetches and stores all results to cache
    pub async fn fetch(&mut self) -> Result<()> {
        let mut tasks = Vec::new();
        let fetcher = Arc::new(RwLock::new(self.clone()));

        for entry in self.iter().into_par_iter() {
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
            let file_name = Url::parse(&key)?.path_segments().unwrap().last().unwrap().to_string();
            let path = dir.join(file_name);
            if !dir.exists(){
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
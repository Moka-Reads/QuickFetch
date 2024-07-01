use crate::package::Config;
use crate::traits::{Entry, EntryKey, EntryValue};
use crate::EncryptionMethod;
use anyhow::Result;
use bytes::{Bytes, BytesMut};
use futures::future::join_all;
use futures::StreamExt;
use log::{debug, error, info};
use notify::{Config as NConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use reqwest::{Client, Response};
use serde::Deserialize;
use sled::{Db, IVec};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::{channel, Receiver};
type ConfigType = crate::package::Mode;

/// WatchFetcher struct
#[derive(Debug, Clone)]
pub struct WatchFetcher<E: Entry> {
    db: Db,
    config_path: PathBuf,
    config_type: ConfigType,
    config: Config<E>,
    db_path: PathBuf,
    client: Client,
    encryption_method: Option<EncryptionMethod>,
}

impl<E: Entry + Clone + Send + Sync + 'static + for<'de> Deserialize<'de>> WatchFetcher<E> {
    pub async fn new<P: AsRef<Path> + Sync>(
        db_path: P,
        config_path: P,
        config_type: ConfigType,
    ) -> Result<Self> {
        let db = sled::open(&db_path)?;
        let config = Config::from_file(&config_path, config_type).await?;
        let client = Client::builder()
            .brotli(true) // by default enable brotli compression
            .build()?;
        let mut wf = Self {
            db,
            config_path: config_path.as_ref().to_path_buf(),
            config_type,
            config,
            db_path: db_path.as_ref().to_path_buf(),
            client,
            encryption_method: None,
        };
        wf.concurrent_fetch().await?;
        Ok(wf)
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
}

// Database Operations
impl<E: Entry + Clone + Send + Sync + 'static> WatchFetcher<E> {
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
/// TODO:
/// - Implement the methods to actually do DB operations
/// - Update, get and remove operations
/// - If file is removed, then clear db
impl<E: Entry + Clone + Send + Sync + 'static + for<'de> Deserialize<'de>> WatchFetcher<E> {
    async fn resp_bytes(&self, response: Response) -> Result<Bytes> {
        let mut stream = response.bytes_stream();
        let mut bytes = BytesMut::new();

        while let Some(item) = stream.next().await {
            let b = item?;
            bytes.extend_from_slice(&b);
        }

        Ok(bytes.freeze())
    }

    fn encrypt_bytes(&self, bytes: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        if let Some(encryption_method) = &self.encryption_method {
            let bytes = encryption_method.encrypt(bytes.as_ref(), key)?;
            Ok(bytes)
        } else {
            Ok(bytes.to_vec())
        }
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

    async fn handle_entry(&self, entry: E) -> Result<()> {
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

    async fn concurrent_fetch(&mut self) -> Result<()> {
        let mut tasks = Vec::new();
        for entry in self.config.packages_owned() {
            let wf = self.clone();
            tasks.push(tokio::spawn(
                async move { wf.handle_entry(entry.clone()).await },
            ));
        }
        join_all(tasks).await.into_iter().try_for_each(|x| x?)?;
        Ok(())
    }

    pub async fn watching(&mut self) {
        println!("Watching {}", &self.config_path.display());
        if let Err(e) = self.watch().await {
            error!("Error: {:?}", e)
        }
    }

    pub fn get_all<V>(&self) -> Vec<V>
    where
        V: EntryValue,
    {
        let mut values = Vec::new();
        for entry in self.db.iter() {
            let entry = entry.unwrap();
            let k = entry.0;
            let v = entry.1;

            let value = Self::dec_or_clone(self.encryption_method, &v, &k).unwrap();
            values.push(V::from_ivec(IVec::from(value)))
        }
        values
    }
}

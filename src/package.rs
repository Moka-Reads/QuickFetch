use std::path::Path;
use anyhow::anyhow;
use crate::Entry;
use bincode;
use semver::Version;
use serde::{Deserialize, Serialize};
use tokio::fs::read_to_string;
use sled::IVec;

/// A Minimal Package Implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    name: String,
    version: String,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    packages: Vec<Package>,
}

enum Mode{
    Json,
    Toml,
}

#[allow(dead_code)]
impl Package {
    pub fn new(name: &str, version: &str, url: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            url: url.to_string(),
        }
    }

    pub fn verify_valid_version(&self) -> bool {
        Version::parse(&self.version).is_ok()
    }
}

#[allow(dead_code)]
impl Config {
    async fn from_file<P: AsRef<Path>>(path: P, mode: Mode) -> anyhow::Result<Self> {
        // read and verify versions
        let contents = read_to_string(path).await?;
        let data: Vec<Package> = match mode {
            Mode::Json => serde_json::from_str(&contents)?,
            Mode::Toml => toml::from_str(&contents)?
        };

        for pkg in &data {
            if !pkg.verify_valid_version() {
                return Err(anyhow!("Invalid Semantic Versioning Format"));
            }
        }

        Ok(Config { packages: data })
    }

    pub async fn from_json_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        Self::from_file(path, Mode::Json).await
    }

    pub async fn from_toml_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        Self::from_file(path, Mode::Toml).await
    }

    pub fn packages(self) -> Vec<Package>{
        self.packages
    }
}


impl From<IVec> for Package{
    fn from(value: IVec) -> Self {
        let bytes = value.as_ref();
        bincode::deserialize(bytes).unwrap()
    }
}

impl Entry for Package {
    fn is_modified(&self, keys_iter: impl DoubleEndedIterator<Item=Result<IVec, sled::Error>>) -> Option<IVec> {
        for key in keys_iter{
            let key = key.unwrap();
            let pkg = Package::from(key.clone());
            if &pkg.name == &self.name && (&self.version != &pkg.version || &self.url != &pkg.url) {
                return Some(key)
            }
        }

        None
    }

    fn url(&self) -> String {
        self.url.to_string()
    }

    fn entry_bytes(&self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }

    fn log_cache(&self) {
        info!("{} v{} (cached)", &self.name, &self.version)
    }

    fn log_caching(&self) {
        info!("{} v{} caching", &self.name, &self.version)
    }
}

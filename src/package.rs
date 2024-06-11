use crate::Entry;
use anyhow::anyhow;
use bincode;
use semver::Version;
use serde::{Deserialize, Serialize};
use sled::IVec;
use std::path::Path;
use tokio::fs::read_to_string;

/// A Minimal Package Implementation
///
/// This module provides a minimal package implementation
/// If you are looking to use Github releases as a source for your packages,
/// you can use the `github_release` method to create a new package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    name: String,
    version: String,
    url: String,
    is_gh: bool, // used to turn off semantic versioning check for github releases
}

/// A Minimal Config Implementation
///
/// The Config struct is used to store a list of packages.
/// We provide methods of reading from both JSON and TOML files, that also verify correct semantic versioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    packages: Vec<Package>,
}

enum Mode {
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
            is_gh: false,
        }
    }

    pub fn github_release(user: &str, repo: &str, tag: &str, asset: &str) -> Self {
        Self {
            // This naming convention is used to avoid conflicts with other packages that may use
            // the same user/repo and tag, but for a different asset file.
            name: format!("{}/{}-{}/{}", user, repo, tag, asset),
            version: tag.to_string(),
            url: format!(
                "https://github.com/{}/{}/releases/download/{}/{}",
                user, repo, tag, asset
            ),
            is_gh: true,
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
        let data: Config = match mode {
            Mode::Json => serde_json::from_str::<Config>(&contents)?,
            Mode::Toml => toml::from_str::<Config>(&contents)?,
        };


        for pkg in &data.packages {
            if !pkg.verify_valid_version() && !pkg.is_gh{
                return Err(anyhow!("Invalid Semantic Versioning Format"));
            }
        }

        Ok(data)
    }

    /// Reads a JSON file and returns a Config struct
    pub async fn from_json_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        Self::from_file(path, Mode::Json).await
    }

    /// Reads a TOML file and returns a Config struct
    pub async fn from_toml_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        Self::from_file(path, Mode::Toml).await
    }

    /// Returns a list of packages
    pub fn packages(self) -> Vec<Package> {
        self.packages
    }
}

impl Entry for Package {
    fn is_modified(
        &self,
        keys_iter: impl DoubleEndedIterator<Item = Result<IVec, sled::Error>>,
    ) -> Option<IVec> {
        for key in keys_iter {
            let key = key.unwrap();
            let pkg = Package::from_ivec(key.clone());
            if &pkg.name == &self.name && (&self.version != &pkg.version || &self.url != &pkg.url) {
                return Some(key);
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

    fn from_ivec(value: IVec) -> Self where Self: Sized {
        bincode::deserialize(value.as_ref()).unwrap()
    }
}

use quickfetch_traits::Entry;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::read_to_string;

use crate::key_val::{GHValue, SimpleValue};
/// A Minimal Package Implementation
///
/// This module provides a minimal package implementation
///
/// It requires:
/// - a name (String)
/// - a semantic version (String)
/// - a URL (String)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplePackage {
    name: String,
    version: String,
    url: String,
}

impl Entry for SimplePackage {
    type Key = String;
    type Value = SimpleValue;

    fn key(&self) -> Self::Key {
        self.name.clone()
    }

    fn value(&self) -> Self::Value {
        SimpleValue::new(self.version.clone(), self.url.clone())
    }
}

/// A Minimal GH Package Implementation
///
/// It requires:
/// - an owner (String)
/// - a repo (String)
/// - a tag (String)
/// - an asset (String)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GHPackage {
    owner: String,
    repo: String,
    tag: String,
    asset: String,
}

impl Entry for GHPackage {
    type Key = String;
    type Value = GHValue;

    fn key(&self) -> Self::Key {
        format!(
            "{} {}/{} [{}]",
            &self.asset, self.owner, self.repo, &self.tag
        )
    }

    fn value(&self) -> Self::Value {
        GHValue::new(
            self.owner.clone(),
            self.repo.clone(),
            self.tag.clone(),
            self.asset.clone(),
        )
    }
}

/// A Minimal Config Implementation
///
/// The Config struct is used to store a list of Packages (generically PK).
/// We provide methods of reading from both JSON and TOML files, that also verify correct semantic versioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config<PK> {
    packages: Vec<PK>,
}

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Json,
    Toml,
}

#[allow(dead_code)]
impl SimplePackage {
    pub fn verify_valid_version(&self) -> bool {
        Version::parse(&self.version).is_ok()
    }
}

#[allow(dead_code)]
impl<PK: Clone> Config<PK> {
    /// Reads a configuration file (JSON or TOML) and returns a Config struct.
    pub async fn from_file<P>(path: P, mode: Mode) -> anyhow::Result<Self>
    where
        P: AsRef<Path> + Send + Sync,
        PK: for<'de> Deserialize<'de>,
    {
        // Read file contents
        let contents = read_to_string(path.as_ref()).await?;

        // Deserialize based on mode
        let data = match mode {
            Mode::Json => serde_json::from_str::<Config<PK>>(&contents)?,
            Mode::Toml => toml::from_str::<Config<PK>>(&contents)?,
        };

        Ok(data)
    }

    /// Reads a JSON file and returns a Config struct.
    pub async fn from_json_file<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path> + Send + Sync,
        PK: for<'de> Deserialize<'de>,
    {
        Self::from_file(path, Mode::Json).await
    }

    /// Reads a TOML file and returns a Config struct.
    pub async fn from_toml_file<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path> + Send + Sync,
        PK: for<'de> Deserialize<'de>,
    {
        Self::from_file(path, Mode::Toml).await
    }

    /// Returns a reference to the list of packages.
    pub fn packages(&self) -> &[PK] {
        &self.packages
    }

    /// Returns an owned list of packages.
    pub fn packages_owned(&self) -> Vec<PK> {
        self.packages.clone()
    }
}

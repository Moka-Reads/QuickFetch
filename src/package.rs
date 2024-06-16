use bincode;
use quickfetch_derive::QFEntry;
use quickfetch_traits::Entry;
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
#[derive(QFEntry, Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    #[mod_eq]
    #[name]
    name: String,
    #[mod_neq]
    #[version]
    version: String,
    #[mod_neq]
    #[url]
    url: String,
}

/// A Github Release Implementation that can be used with a config file
/// and then converts to `Package` struct.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GithubReleaseBuilder {
    user: String,
    repo: String,
    asset: String,
    tag: String,
}

impl From<GithubReleaseBuilder> for Package {
    fn from(value: GithubReleaseBuilder) -> Self {
        let url = format!(
            "https://github.com/{}/{}/releases/download/{}/{}",
            &value.user, &value.repo, &value.tag, &value.asset
        );
        let name = format!("{}/{} - {}", &value.user, &value.repo, &value.asset);
        Package::new(&name, &value.tag, &url)
    }
}

pub fn github_to_packages(v: Vec<GithubReleaseBuilder>) -> Vec<Package> {
    v.into_iter().map(Package::from).collect()
}

/// A Minimal Config Implementation
///
/// The Config struct is used to store a list of Packages (generically PK).
/// We provide methods of reading from both JSON and TOML files, that also verify correct semantic versioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config<PK> {
    packages: Vec<PK>,
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
        }
    }

    pub fn verify_valid_version(&self) -> bool {
        Version::parse(&self.version).is_ok()
    }
}

#[allow(dead_code)]
impl<PK> Config<PK> {
    /// Reads a configuration file (JSON or TOML) and returns a Config struct.
    async fn from_file<P>(path: P, mode: Mode) -> anyhow::Result<Self>
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
}

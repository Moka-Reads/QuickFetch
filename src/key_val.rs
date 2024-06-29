use quickfetch_traits::EntryValue;
use serde::{Deserialize, Serialize};
use sled::IVec;
use std::borrow::Cow;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SimpleValue {
    version: String,
    url: String,
    response: Vec<u8>,
}

impl SimpleValue {
    pub fn new(version: String, url: String) -> Self {
        Self {
            version,
            url,
            response: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GHValue {
    owner: String,
    repo: String,
    tag: String,
    asset: String,
    response: Vec<u8>,
}

impl GHValue {
    pub fn new(owner: String, repo: String, tag: String, asset: String) -> Self {
        Self {
            owner,
            repo,
            tag,
            asset,
            response: Vec::new(),
        }
    }

    pub fn fmt_url(&self) -> String {
        format!(
            "https://github.com/{}/{}/releases/download/{}/{}",
            &self.owner, &self.repo, &self.tag, &self.asset
        )
    }
}

impl EntryValue for SimpleValue {
    fn bytes(&self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }

    fn from_ivec(value: IVec) -> Self
    where
        Self: Sized,
    {
        bincode::deserialize(&value).unwrap()
    }

    fn url(&self) -> String {
        self.url.clone()
    }

    fn response(&self) -> Cow<'_, [u8]> {
        Cow::from(&self.response)
    }

    fn set_response(&mut self, response: &[u8]) {
        self.response = response.to_vec();
    }

    fn is_same(&self, other: &Self) -> bool {
        self.version == other.version
    }
}

impl EntryValue for GHValue {
    fn bytes(&self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }

    fn from_ivec(value: IVec) -> Self
    where
        Self: Sized,
    {
        bincode::deserialize(&value).unwrap()
    }

    fn url(&self) -> String {
        self.fmt_url()
    }

    fn response(&self) -> Cow<'_, [u8]> {
        Cow::from(&self.response)
    }

    fn set_response(&mut self, response: &[u8]) {
        self.response = response.to_vec();
    }

    fn is_same(&self, other: &Self) -> bool
    where
        Self: Sized,
    {
        self.owner == other.owner
            && self.repo == other.repo
            && self.tag == other.tag
            && self.asset == other.asset
    }
}

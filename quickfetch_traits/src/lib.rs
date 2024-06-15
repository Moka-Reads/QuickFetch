#[macro_use] extern crate log;
use sled::IVec;

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
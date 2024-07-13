#[macro_use]
extern crate log;
use aead::{generic_array::GenericArray, Aead, KeyInit};
use anyhow::Result;
use rand::{rngs::OsRng, RngCore};
use sled::IVec;
use std::borrow::Cow;
use std::fmt::Display;
/// Entry trait that will be used to be able to fetch and cache data as the Key
pub trait Entry {
    type Key: EntryKey + Send + Sync;
    type Value: EntryValue + Send + Sync;

    fn key(&self) -> Self::Key;
    fn value(&self) -> Self::Value;
}

pub trait EntryKey: Display {
    fn bytes(&self) -> Vec<u8>;
    fn from_ivec(value: IVec) -> Self
    where
        Self: Sized;
    fn log_cache(&self) {
        info!("{} (cached)", self)
    }
    fn log_caching(&self) {
        info!("{} caching", self)
    }
}

impl EntryKey for String {
    fn bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    fn from_ivec(value: IVec) -> Self
    where
        Self: Sized,
    {
        String::from_utf8(value.to_vec()).unwrap()
    }
}

pub trait EntryValue {
    /// Convert the value to bytes
    fn bytes(&self) -> Vec<u8>;
    /// Convert the value from IVec
    fn from_ivec(value: IVec) -> Self
    where
        Self: Sized;
    /// Return the url to send the request
    fn url(&self) -> String;
    /// Return the response as a Copy on Write byte array
    fn response(&self) -> Cow<'_, [u8]>;
    /// Set the response from the request as a byte array
    fn set_response(&mut self, response: &[u8]);
    /// Check if the value is the same as another value (excluding the response)
    fn is_same(&self, other: &Self) -> bool
    where
        Self: Sized;
}

const NONCE_SIZE: usize = 12;

pub trait EncryptionMethod {
    type Cipher: Aead + KeyInit;

    fn new_cipher(key: &[u8]) -> Result<Self::Cipher>;

    fn encrypt(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        encrypt_generic::<Self::Cipher>(data, key)
    }

    fn decrypt(&self, data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
        decrypt_generic::<Self::Cipher>(data, key)
    }
}

fn encrypt_generic<C: Aead>(data: &[u8], key: &[u8]) -> Result<Vec<u8>>
where
    C: KeyInit,
{
    let cipher = C::new_from_slice(key).unwrap();
    let mut nonce = vec![0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce);
    let nonce = GenericArray::from_slice(&nonce);
    let ciphertext = cipher.encrypt(nonce, data).unwrap();
    Ok(ciphertext)
}

fn decrypt_generic<C: Aead>(data: &[u8], key: &[u8]) -> Result<Vec<u8>>
where
    C: KeyInit,
{
    let cipher = C::new_from_slice(key).unwrap();
    let (nonce, ciphertext) = data.split_at(NONCE_SIZE);
    let nonce = GenericArray::from_slice(nonce);
    let plaintext = cipher.decrypt(nonce, ciphertext).unwrap();
    Ok(plaintext)
}

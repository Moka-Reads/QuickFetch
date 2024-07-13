#![allow(unused_imports)]
use aead::generic_array::GenericArray;
use anyhow::Result;
use quickfetch_traits::EncryptionMethod;

#[cfg(feature = "aes")]
#[derive(Debug, Copy, Clone)]
pub struct AESGCM;

#[cfg(feature = "aes")]
impl EncryptionMethod for AESGCM {
    type Cipher = aes_gcm::Aes256Gcm;

    fn new_cipher(key: &[u8]) -> Result<Self::Cipher> {
        use aes_gcm::KeyInit;
        Ok(Self::Cipher::new(GenericArray::from_slice(key)))
    }
}

#[cfg(feature = "chacha20poly")]
#[derive(Debug, Copy, Clone)]
pub struct ChaCha20Poly;

#[cfg(feature = "chacha20poly")]
impl EncryptionMethod for ChaCha20Poly {
    type Cipher = chacha20poly1305::ChaCha20Poly1305;

    fn new_cipher(key: &[u8]) -> Result<Self::Cipher> {
        use chacha20poly1305::KeyInit;
        Ok(Self::Cipher::new(GenericArray::from_slice(key)))
    }
}

#[cfg(feature = "aes-gcm-siv")]
#[derive(Debug, Copy, Clone)]
pub struct AESGCMSIV;

#[cfg(feature = "aes-gcm-siv")]
impl EncryptionMethod for AESGCMSIV {
    type Cipher = aes_gcm_siv::Aes256GcmSiv;

    fn new_cipher(key: &[u8]) -> Result<Self::Cipher> {
        use aes_gcm_siv::KeyInit;
        Ok(Self::Cipher::new(GenericArray::from_slice(key)))
    }
}

#[cfg(feature = "aes-siv")]
#[derive(Debug, Copy, Clone)]
pub struct AESSIV;

#[cfg(feature = "aes-siv")]
impl EncryptionMethod for AESSIV {
    type Cipher = aes_siv::Aes256SivAead;

    fn new_cipher(key: &[u8]) -> Result<Self::Cipher> {
        use aes_siv::KeyInit;
        Ok(Self::Cipher::new(GenericArray::from_slice(key)))
    }
}

#[cfg(feature = "ascon-aead")]
#[derive(Debug, Copy, Clone)]
pub struct Ascon;

#[cfg(feature = "ascon-aead")]
impl EncryptionMethod for Ascon {
    type Cipher = ascon_aead::Ascon128;

    fn new_cipher(key: &[u8]) -> Result<Self::Cipher> {
        use aead::KeyInit;
        Ok(Self::Cipher::new(GenericArray::from_slice(key)))
    }
}

#[cfg(feature = "ccm")]
#[derive(Debug, Copy, Clone)]
pub struct CCM;

#[cfg(feature = "ccm")]
impl EncryptionMethod for CCM {
    type Cipher = ccm::Ccm<aes::Aes256, ccm::consts::U10, ccm::consts::U13>;

    fn new_cipher(key: &[u8]) -> Result<Self::Cipher> {
        use aead::KeyInit;
        Ok(Self::Cipher::new(GenericArray::from_slice(key)))
    }
}

#[cfg(feature = "deoxys")]
#[derive(Debug, Copy, Clone)]
pub struct Deoxys;

#[cfg(feature = "deoxys")]
impl EncryptionMethod for Deoxys {
    type Cipher = deoxys::DeoxysII256;

    fn new_cipher(key: &[u8]) -> Result<Self::Cipher> {
        use aead::KeyInit;
        Ok(Self::Cipher::new(GenericArray::from_slice(key)))
    }
}

#[cfg(feature = "eax")]
#[derive(Debug, Copy, Clone)]
pub struct EAX;

#[cfg(feature = "eax")]
impl EncryptionMethod for EAX {
    type Cipher = eax::Eax<aes::Aes256>;

    fn new_cipher(key: &[u8]) -> Result<Self::Cipher> {
        use aead::KeyInit;
        Ok(Self::Cipher::new(GenericArray::from_slice(key)))
    }
}

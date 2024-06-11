use aes_gcm::{aead::Aead, AeadCore, Aes256Gcm, KeyInit, Nonce};
use rand::rngs::OsRng;

#[derive(Debug, Clone)]
pub enum EncryptionMethod {
    AesGcm,
}

impl EncryptionMethod {
    pub fn encrypt(&self, data: &[u8], key: &[u8]) -> anyhow::Result<Vec<u8>> {
        match self {
            EncryptionMethod::AesGcm => {
                let cipher = Aes256Gcm::new_from_slice(key).unwrap();
                let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

                let ciphertext = cipher.encrypt(&nonce, data).unwrap();

                // Prepend the nonce to the ciphertext
                let mut encrypted_data = nonce.to_vec();
                encrypted_data.extend_from_slice(&ciphertext);

                Ok(encrypted_data)
            }
        }
    }

    pub fn decrypt(&self, data: &[u8], key: &[u8]) -> anyhow::Result<Vec<u8>> {
        match self {
            EncryptionMethod::AesGcm => {
                let cipher = Aes256Gcm::new_from_slice(key).unwrap();

                // Split the nonce and ciphertext
                let (nonce, ciphertext) = data.split_at(12);

                let plaintext = cipher.decrypt(&Nonce::from_slice(nonce), ciphertext).unwrap();

                Ok(plaintext)
            }
        }
    }
}
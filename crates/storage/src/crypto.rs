use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use synchalo_core::AppError;

pub const DATA_KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 24;

pub fn generate_data_key() -> Result<[u8; DATA_KEY_BYTES], AppError> {
    let mut key = [0_u8; DATA_KEY_BYTES];
    getrandom::fill(&mut key).map_err(|_| AppError::Crypto)?;
    Ok(key)
}

pub fn encode_data_key(key: &[u8; DATA_KEY_BYTES]) -> String {
    STANDARD_NO_PAD.encode(key)
}

pub fn decode_data_key(encoded: &str) -> Result<[u8; DATA_KEY_BYTES], AppError> {
    let bytes = STANDARD_NO_PAD
        .decode(encoded)
        .map_err(|_| AppError::Crypto)?;
    bytes.try_into().map_err(|_| AppError::Crypto)
}

#[derive(Clone)]
pub(crate) struct CryptoBox {
    cipher: XChaCha20Poly1305,
}

impl CryptoBox {
    pub(crate) fn new(key: &[u8; DATA_KEY_BYTES]) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(key.into()),
        }
    }

    pub(crate) fn encrypt(
        &self,
        plaintext: &[u8],
        associated_data: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), AppError> {
        let mut nonce = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| AppError::Crypto)?;
        let nonce = XNonce::try_from(nonce.as_slice()).map_err(|_| AppError::Crypto)?;
        let ciphertext = self
            .cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad: associated_data,
                },
            )
            .map_err(|_| AppError::Crypto)?;
        Ok((nonce.to_vec(), ciphertext))
    }

    pub(crate) fn decrypt(
        &self,
        nonce: &[u8],
        ciphertext: &[u8],
        associated_data: &[u8],
    ) -> Result<Vec<u8>, AppError> {
        if nonce.len() != NONCE_BYTES {
            return Err(AppError::Crypto);
        }
        let nonce = XNonce::try_from(nonce).map_err(|_| AppError::Crypto)?;
        self.cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ciphertext,
                    aad: associated_data,
                },
            )
            .map_err(|_| AppError::Crypto)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ciphertext_is_bound_to_record_id() {
        let crypto = CryptoBox::new(&[7_u8; DATA_KEY_BYTES]);
        let (nonce, ciphertext) = crypto.encrypt(b"secret", b"record-a").unwrap();

        assert_eq!(
            crypto.decrypt(&nonce, &ciphertext, b"record-a").unwrap(),
            b"secret"
        );
        assert!(crypto.decrypt(&nonce, &ciphertext, b"record-b").is_err());
    }
}

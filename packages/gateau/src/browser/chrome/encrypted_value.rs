//! This module contains functions to decrypt the value of a cookie
//! encrypted by Chrome on Unix, macOS and Windows platforms.

#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod mac;
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod posix;
#[cfg(windows)]
pub(crate) mod windows;

#[derive(Debug, thiserror::Error)]
pub enum DecryptError {
    #[error("Failed to decrypt value due to invalid input/key length")]
    InvalidInputLength,

    #[error("Failed to decrypt value")]
    InvalidInput,

    #[error("Failed to decrypt value due to invalid UTF-8")]
    InvalidUtf8 {
        #[from]
        source: std::string::FromUtf8Error,
    },
}

/// Decrypts a cookie value encrypted by Chrome on Unix platforms (including macOS)
/// (with AES-128-CBC).
#[cfg(unix)]
pub(crate) fn decrypt_value<K: AsRef<[u8]>, V: AsRef<[u8]>>(
    key: K,
    encrypted_value: V,
) -> Result<String, DecryptError> {
    use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};

    /// Size of initialization vector for AES 128-bit blocks.
    const IVBLOCK_SIZE_AES128: usize = 16;

    type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

    // Chrome's initialization vector.
    const IV: [u8; IVBLOCK_SIZE_AES128] = [b' '; IVBLOCK_SIZE_AES128];

    let mut output_buffer = vec![0u8; encrypted_value.as_ref().len()];

    let value = Aes128CbcDec::new(key.as_ref().into(), &IV.into())
        .decrypt_padded_b2b_mut::<Pkcs7>(encrypted_value.as_ref(), output_buffer.as_mut())
        .map_err(|_| DecryptError::InvalidInputLength)?;

    Ok(String::from_utf8(value.into())?)
}

/// Decrypts a cookie value encrypted by Chrome on Windows
/// (with AES-256-GCM).
#[cfg(windows)]
pub(crate) fn decrypt_value<K: AsRef<[u8]>, V: AsRef<[u8]>>(
    key: K,
    encrypted_value: V,
) -> Result<String, DecryptError> {
    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit};

    /// Size of the nonce for AES 256-bit.
    const AEAD_NONCE_SIZE: usize = 96 / 8;

    let cipher = Aes256Gcm::new(key.as_ref().into());

    let nonce = encrypted_value
        .as_ref()
        .get(..AEAD_NONCE_SIZE)
        .ok_or_else(|| DecryptError::InvalidInputLength)?;

    let ciphertext = encrypted_value
        .as_ref()
        .get(AEAD_NONCE_SIZE..)
        .ok_or_else(|| DecryptError::InvalidInputLength)?;

    Ok(String::from_utf8(
        cipher
            .decrypt(nonce.into(), ciphertext)
            .map_err(|_| DecryptError::InvalidInput)?,
    )?)
}

//! This module contains functions to decrypt the value of a cookie
//! encrypted by Chrome on Unix, macOS and Windows platforms.

#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod mac;
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod posix;
#[cfg(target_os = "windows")]
pub(crate) mod windows;

/// Decrypts a cookie value encrypted by Chrome on Unix platforms
/// (with AES-128-CBC).
#[cfg(unix)]
pub(crate) fn decrypt_value<K: AsRef<[u8]>, V: AsRef<[u8]>>(
    key: K,
    encrypted_value: V,
) -> color_eyre::Result<String> {
    use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};

    /// Size of initialization vector for AES 128-bit blocks.
    const IVBLOCK_SIZE_AES128: usize = 16;

    type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

    // Chrome's initialization vector.
    const IV: [u8; IVBLOCK_SIZE_AES128] = [b' '; IVBLOCK_SIZE_AES128];

    let mut output_buffer = vec![0u8; encrypted_value.as_ref().len()];

    let value = Aes128CbcDec::new(key.as_ref().into(), &IV.into())
        .decrypt_padded_b2b_mut::<Pkcs7>(encrypted_value.as_ref(), output_buffer.as_mut())?;

    Ok(String::from_utf8(value.into())?)
}

/// Decrypts a cookie value encrypted by Chrome on Windows
/// (with AES-256-GCM).
#[cfg(target_os = "windows")]
pub(crate) fn decrypt_value<K: AsRef<[u8]>, V: AsRef<[u8]>>(
    key: K,
    encrypted_value: V,
) -> color_eyre::Result<String> {
    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit};
    use color_eyre::eyre::eyre;

    /// Size of the nonce for AES 256-bit.
    const AEAD_NONCE_SIZE: usize = 96 / 8;

    let cipher = Aes256Gcm::new(key.as_ref().into());

    let nonce = encrypted_value
        .as_ref()
        .get(..AEAD_NONCE_SIZE)
        .ok_or_else(|| eyre!("Failed to get nonce, value is too short to contain one"))?;

    let ciphertext = encrypted_value
        .as_ref()
        .get(AEAD_NONCE_SIZE..)
        .ok_or_else(|| eyre!("Failed to get ciphertext, value is too short to contain one"))?;

    Ok(String::from_utf8(
        cipher
            .decrypt(nonce.into(), ciphertext)
            .map_err(|_| eyre!("Failed to decrypt value"))?,
    )?)
}

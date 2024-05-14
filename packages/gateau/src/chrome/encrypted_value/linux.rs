//! Linux-specific functions to get the key used to encrypt cookies in Chrome.
//! On Linux, cookies are encrypted using the AES 128-bit algorithm and CBC mode,
//! and the password from which is derived the key used to encrypt the cookie is either:
//! - stored on the keyring, if there is an available one,
//! - or "peanuts" (the default key used by Chrome on Linux).

use std::collections::HashMap;

use keyring::{
    credential::{LinuxCredential, PlatformCredential},
    Entry,
};
use pbkdf2::{
    password_hash::{PasswordHasher, SaltString},
    Algorithm, Params, Pbkdf2,
};

use crate::chrome::ChromeVariant;

/// Salt for symmetric key derivation.
const SYMMETRIC_SALT: &[u8] = b"saltysalt";

/// Number of iterations to hash the password to
/// obtain the key used to encrypt cookies.
const HASH_ROUNDS: u32 = 1;

/// Length of the derived key used by Chrome for AES-128.
const DERIVED_KEY_LENGTH: usize = 128;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to get password from keyring")]
    Keyring(#[from] keyring::Error),
    #[error("Failed to hash password")]
    Pbkdf2(#[from] pbkdf2::password_hash::Error),
}

type Result<T> = std::result::Result<T, Error>;

/// Derives a key from a password using the same parameters as Chrome for
/// Linux platform.
fn derive_key_from_password<P: AsRef<[u8]>>(password: P) -> Result<Vec<u8>> {
    let salt = SaltString::encode_b64(SYMMETRIC_SALT)?;

    let key = Pbkdf2.hash_password_customized(
        password.as_ref(),
        Some(Algorithm::Pbkdf2Sha1.ident()),
        None,
        Params {
            rounds: HASH_ROUNDS,
            output_length: DERIVED_KEY_LENGTH / 8,
        },
        &salt,
    )?;

    Ok(key.hash.unwrap().as_bytes().to_vec())
}

/// Gets the password used to encrypt cookies in Chrome on Linux using the
/// the secret service API.
fn get_v11_password(variant: ChromeVariant) -> Result<String> {
    let variant = match variant {
        ChromeVariant::Chromium => "chromium",
        ChromeVariant::Chrome => "chrome",
        ChromeVariant::Edge => "edge",
    };
    let credential = PlatformCredential::Linux(LinuxCredential {
        collection: String::from("default"),
        attributes: HashMap::from([(String::from("application"), String::from(variant))]),
        label: String::new(),
    });
    let entry = Entry::new_with_credential(&credential)?;

    Ok(entry.get_password()?)
}

/// Gets the key used to encrypt cookies in Chrome on Linux by deriving it from
/// the password retrieved with the secret service API.
pub(crate) fn get_v11_key(variant: ChromeVariant) -> Result<Vec<u8>> {
    let password = get_v11_password(variant)?;
    derive_key_from_password(password)
}

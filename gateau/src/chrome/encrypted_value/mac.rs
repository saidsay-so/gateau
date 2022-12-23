use cached::proc_macro::cached;
use keyring::{
    credential::{MacCredential, MacKeychainDomain, PlatformCredential},
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
const HASH_ROUNDS: u32 = 1003;

/// Length of the derived key used by Chrome for AES-128.
const DERIVED_KEY_LENGTH: usize = 128;

/// Gets the password used to encrypt cookies in Chrome on macOS using the
/// the keychain API.
pub(crate) fn get_v10_password(variant: ChromeVariant) -> color_eyre::Result<String> {
    let (service, account) = match variant {
        ChromeVariant::Chromium => ("Chromium Safe Storage", "Chromium"),
        ChromeVariant::Chrome => ("Chrome Safe Storage", "Chrome"),
    };

    let credential = PlatformCredential::Mac(MacCredential {
        service: String::from(service),
        account: String::from(account),
        domain: MacKeychainDomain::User,
    });

    let entry = Entry::new_with_credential(&credential)?;

    Ok(entry.get_password()?)
}

/// Derives a key from a password using the same parameters as Chrome for
/// macOS platform.
fn derive_key_from_password<P: AsRef<[u8]>>(password: P) -> color_eyre::Result<Vec<u8>> {
    let salt = SaltString::b64_encode(SYMMETRIC_SALT)?;

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

/// Gets the key used to encrypt cookies in Chrome on macOS.
#[cached(result)]
pub(crate) fn get_v10_key(variant: ChromeVariant) -> color_eyre::Result<Vec<u8>> {
    let password = get_v10_password(variant)?;
    derive_key_from_password(password)
}

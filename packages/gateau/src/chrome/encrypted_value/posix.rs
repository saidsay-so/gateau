//! Unix-specific functions to get the key used to encrypt cookies in Chrome.
//! On Unix systems, cookies are encrypted using the AES 128-bit algorithm and CBC mode,
//! and the password from which is derived the key used to encrypt the cookie is "peanuts".

/// Default password used by Chrome on Linux when no keyring is available or on other Unix platforms except macOS.
pub const _CHROME_V10_PASSWORD: &str = "peanuts";

/// Default key used by Chrome on Linux when no keyring is available.
/// This is the result of deriving the key from the default ("peanuts") password (see notebook).
/// To avoid having to derive the key every time, we just hardcode it.
pub const CHROME_V10_KEY: [u8; 16] = [
    253, 98, 31, 229, 162, 180, 2, 83, 157, 250, 20, 124, 169, 39, 39, 120,
];

#[cfg(test)]
mod test {
    use crate::chrome::encrypted_value::decrypt_value;

    use super::*;

    #[test]
    fn test_chrome_v10_key() {
        const ENCRYPTED_EXAMPLE: &[u8] = &[
            0x76, 0x31, 0x30, 0xe9, 0xbf, 0x20, 0xc4, 0xcf, 0xaa, 0xa2, 0xfa, 0x8d, 0xf3, 0x3a,
            0x42, 0x60, 0x42, 0x4e, 0x5b,
        ];

        assert_eq!(
            decrypt_value(CHROME_V10_KEY, &ENCRYPTED_EXAMPLE[3..]).unwrap(),
            "PENDING+400"
        );
    }
}

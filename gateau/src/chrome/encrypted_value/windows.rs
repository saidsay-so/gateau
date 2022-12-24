use base64ct::{Base64, Encoding};
use color_eyre::eyre::ensure;
use windows::Win32::{
    Security::Cryptography::{CryptUnprotectData, CRYPTOAPI_BLOB},
    System::Memory::LocalFree,
};

/// Decrypts a value encrypted with the Windows DPAPI.
///
/// # Safety
///
/// For the function call to be safe, `encrypted_value` must be a valid buffer for the entire duration of the call,
/// which is normally guaranteed by the borrow checker.
#[allow(unsafe_code)]
pub(crate) fn decrypt_dpapi(encrypted_value: &mut [u8]) -> color_eyre::Result<Vec<u8>> {
    let data_in = CRYPTOAPI_BLOB {
        cbData: u32::try_from(encrypted_value.len())?,
        pbData: encrypted_value.as_mut_ptr(),
    };

    let mut data_out = CRYPTOAPI_BLOB::default();

    // SAFETY: `CryptUnprotectData` is a Windows API function whcih is safe to call with the correct parameters.
    // See https://docs.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptunprotectdata
    // and https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Security/Cryptography/fn.CryptUnprotectData.html
    // for more information.
    // We assume that `encrypted_value` is a valid buffer for the duration of the call.
    // We check that `data_out.pbData` is not null before creating the `Vec` and that `CryptUnprotectData` returns a success code.
    unsafe {
        CryptUnprotectData(&data_in, None, None, None, None, 0, &mut data_out).ok()?;

        ensure!(!data_out.pbData.is_null(), "CryptUnprotectData failed");

        let data = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
        LocalFree(data_out.pbData as _);

        Ok(data)
    }
}

/// Get encrypted key (prefixed with [`DPAPI_PREFIX`]) from `local_state` if it exists.
pub(crate) fn get_encrypted_key(
    local_state: &crate::chrome::LocalState,
) -> color_eyre::Result<Option<String>> {
    let os_crypt = local_state
        .values
        .get("os_crypt")
        .map(|obj| obj.as_object())
        .ok_or_else(|| color_eyre::eyre::eyre!("'os_crypt' is not an object"))?;

    let key = os_crypt.and_then(|os_crypt| {
        os_crypt
            .get("encrypted_key")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
    });

    Ok(key)
}

/// Decrypts the key encrypted with DPAPI and encoded in Base64.
pub(crate) fn decrypt_dpapi_encrypted_key<S: AsRef<str>>(
    encrypted_key: S,
) -> color_eyre::Result<Vec<u8>> {
    /// Prefix for encrypted keys in the Local State file.
    const DPAPI_PREFIX: &[u8] = b"DPAPI";

    let mut encrypted_key = Base64::decode_vec(encrypted_key.as_ref())?;
    ensure!(
        encrypted_key.starts_with(DPAPI_PREFIX),
        "invalid encrypted key"
    );
    let mut stripped_encrypted_key = encrypted_key.get_mut(DPAPI_PREFIX.len() - 1..).unwrap();

    decrypt_dpapi(&mut stripped_encrypted_key)
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_get_encrypted_key() {
        let local_state = Cursor::new(
            r#"{
            "os_crypt": {
                "encrypted_key": "expected",
                "ee": "unexpected"
            }
        }"#,
        );
        let encrypted_key = get_encrypted_key(local_state).unwrap();
        assert_eq!(encrypted_key, Some(String::from("expected")));
    }
}

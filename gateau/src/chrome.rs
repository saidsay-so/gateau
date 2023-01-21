//! Cookies management for Chrome and its derivatives.
//!
//! ### Scheme (v18)
//!
//! ```sql
//! CREATE TABLE meta
//!   (
//!      key   LONGVARCHAR NOT NULL UNIQUE PRIMARY KEY,
//!      value LONGVARCHAR
//!   );
//!
//! CREATE TABLE cookies
//!   (
//!      creation_utc       INTEGER NOT NULL,
//!      host_key           TEXT NOT NULL,
//!      top_frame_site_key TEXT NOT NULL,
//!      name               TEXT NOT NULL,
//!      value              TEXT NOT NULL,
//!      encrypted_value    BLOB NOT NULL,
//!      path               TEXT NOT NULL,
//!      expires_utc        INTEGER NOT NULL,
//!      is_secure          INTEGER NOT NULL,
//!      is_httponly        INTEGER NOT NULL,
//!      last_access_utc    INTEGER NOT NULL,
//!      has_expires        INTEGER NOT NULL,
//!      is_persistent      INTEGER NOT NULL,
//!      priority           INTEGER NOT NULL,
//!      samesite           INTEGER NOT NULL,
//!      source_scheme      INTEGER NOT NULL,
//!      source_port        INTEGER NOT NULL,
//!      is_same_party      INTEGER NOT NULL,
//!      last_update_utc    INTEGER NOT NULL
//!   );
//!
//! CREATE UNIQUE INDEX cookies_unique_index
//!   ON cookies(host_key, top_frame_site_key, NAME, path);
//! ```
//!
use std::collections::HashMap;

use color_eyre::eyre::Context;
use cookie::{time::OffsetDateTime, Cookie, CookieBuilder, Expiration, SameSite};
use once_cell::sync::OnceCell;
use rusqlite::Connection;
use serde::Deserialize;

#[cfg(all(unix, not(target_os = "macos")))]
use self::encrypted_value::posix;

#[cfg(target_os = "linux")]
use self::encrypted_value::linux;

#[cfg(target_os = "macos")]
use self::encrypted_value::mac;

#[cfg(windows)]
use {self::encrypted_value::windows, std::path::Path};

pub(crate) mod encrypted_value;
pub(crate) mod paths;

use self::paths::PathProvider;

/// Local state stored in `Local State` file.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct LocalState {
    #[serde(flatten)]
    values: HashMap<String, serde_json::Value>,
}

struct ChromeCookie {
    name: String,
    value: String,
    encrypted_value: Vec<u8>,
    host: String,
    path: String,
    expires: i64,
    secure: bool,
    same_site: i64,
    http_only: bool,
}

// TODO: Do we need support for multiple variants at the same time?
// struct ChromeCacheKeyVariants {
//     chromium: OnceCell<Vec<u8>>,
//     chrome: OnceCell<Vec<u8>>,
// }

// impl ChromeCacheKeyVariants {
//     const fn default() -> Self {
//         Self {
//             chromium: OnceCell::new(),
//             chrome: OnceCell::new(),
//         }
//     }
// }

// impl Index<ChromeVariant> for ChromeCacheKeyVariants {
//     type Output = OnceCell<Vec<u8>>;

//     fn index(&self, index: ChromeVariant) -> &Self::Output {
//         match index {
//             ChromeVariant::Chromium => &self.chromium,
//             ChromeVariant::Chrome => &self.chrome,
//         }
//     }
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChromeVariant {
    Chromium,
    Chrome,
    Edge,
}

// Offset of UNIX epoch (1970-01-01 00:00:00 UTC) from Windows FILETIME epoch
// (1601-01-01 00:00:00 UTC), in microseconds. This value is derived from the
// following: ((1970-1601)*365+89)*24*60*60*1000*1000, where 89 is the number
// of leap year days between 1601 and 1970: (1970-1601)/4 excluding 1700,
// 1800, and 1900.
const WINDOWS_UNIX_EPOCH_OFFSET_MICROS: i64 = 11644473600000000;

// From Chromium source code:
// Time is stored internally as microseconds
// since the Windows epoch (1601-01-01 00:00:00 UTC).
/// Convert a Chrome timestamp (based on Windows epoch) in microseconds
/// to a UNIX timestamp (based on UNIX epoch) in nanoseconds.
fn chrome_to_unix_timestamp_nanos(chrome_time: i64) -> i128 {
    const WINDOWS_UNIX_EPOCH_OFFSET_NANOS: i128 = WINDOWS_UNIX_EPOCH_OFFSET_MICROS as i128 * 1000;

    let nanos = chrome_time as i128 * 1000;

    nanos - WINDOWS_UNIX_EPOCH_OFFSET_NANOS
}

/// Decrypt a cookie value.
#[cfg(all(unix, not(target_os = "macos")))]
fn decrypt_cookie_value<V: AsRef<[u8]>>(
    encrypted_value: V,
    variant: ChromeVariant,
) -> color_eyre::Result<String> {
    /// Length of the header of the encrypted value, if present.
    const HEADER_LEN: usize = 3;

    let encrypted_value = encrypted_value.as_ref();

    static KEY_CACHE: OnceCell<Vec<u8>> = OnceCell::new();

    let key = match encrypted_value.get(..HEADER_LEN) {
        #[cfg(target_os = "linux")]
        Some(b"v11") => Some(
            KEY_CACHE
                .get_or_try_init(|| linux::get_v11_key(variant))?
                .as_slice(),
        ),
        #[cfg(not(target_os = "linux"))]
        Some(b"v11") => unimplemented!("v11 key is not implemented for this platform"),
        Some(b"v10") => Some(posix::CHROME_V10_KEY.as_slice()),
        _ => None,
    };

    if let Some(key) = key {
        encrypted_value::decrypt_value(
            key,
            encrypted_value
                .get(HEADER_LEN..)
                .expect("No data after the header"),
        )
    } else {
        // We assume that it's not encrypted
        String::from_utf8(encrypted_value.into())
            .wrap_err("Failed to decode cookie value as unencrypted")
    }
}

/// Decrypt a cookie value.
#[cfg(target_os = "macos")]
fn decrypt_cookie_value<V: AsRef<[u8]>>(
    encrypted_value: V,
    variant: ChromeVariant,
) -> color_eyre::Result<String> {
    let encrypted_value = encrypted_value.as_ref();

    /// Length of the header of the encrypted value, if present.
    const HEADER_LEN: usize = 3;

    static KEY_CACHE: OnceCell<Vec<u8>> = OnceCell::new();

    let key = match encrypted_value.get(..HEADER_LEN) {
        Some(b"v10") => Some(KEY_CACHE.get_or_try_init(|| mac::get_v10_key(variant))?),
        _ => None,
    };

    if let Some(key) = key {
        encrypted_value::decrypt_value(
            key,
            encrypted_value
                .get(HEADER_LEN..)
                .expect("No data after the header"),
        )
    } else {
        // We assume that it's not encrypted
        String::from_utf8(encrypted_value.into())
            .wrap_err("Failed to decode cookie value as unencrypted")
    }
}

/// Decrypt a cookie value.
#[cfg(windows)]
fn decrypt_cookie_value<V: AsRef<[u8]> + AsMut<[u8]>, P: AsRef<Path>>(
    mut encrypted_value: V,
    local_state: P,
) -> color_eyre::Result<String> {
    let encrypted_value_ref = encrypted_value.as_ref();

    /// Length of the header of the encrypted value, if present.
    const HEADER_LEN: usize = 3;

    static KEY_CACHE: OnceCell<Vec<u8>> = OnceCell::new();

    let key = match encrypted_value_ref.get(..HEADER_LEN) {
        Some(b"v10") => Some(KEY_CACHE.get_or_try_init(|| {
            let local_state = get_local_state(local_state.as_ref())?;

            let encrypted_key = windows::get_encrypted_key(&local_state).ok_or_else(|| {
                color_eyre::eyre::eyre!("Encrypted key is not available in the local state")
            })?;
            windows::decrypt_dpapi_encrypted_key(encrypted_key)
        })?),
        _ => None,
    };

    if let Some(key) = key {
        encrypted_value::decrypt_value(
            key,
            encrypted_value_ref
                .get(HEADER_LEN..)
                .expect("No data after the header"),
        )
    } else {
        // Values seems to be always encrypted on Windows, at least with DPAPI
        // if not with AES-256-GCM
        String::from_utf8(windows::decrypt_dpapi(encrypted_value.as_mut())?)
            .wrap_err("Failed to decode cookie value as UTF-8")
    }
}

#[cfg(windows)]
fn get_local_state(path: &Path) -> color_eyre::Result<LocalState> {
    Ok(serde_json::from_reader(std::io::BufReader::new(
        std::fs::File::open(path)?,
    ))?)
}

/// Get cookies from the database.
#[allow(unused_variables)]
pub(crate) fn get_cookies(
    conn: &Connection,
    variant: ChromeVariant,
    path_provider: PathProvider,
) -> color_eyre::Result<Vec<Cookie<'static>>> {
    let query = "SELECT name, value, encrypted_value, 
                        host_key, path, expires_utc, 
                        is_secure, samesite, is_httponly
        FROM cookies
        WHERE host_filter(host_key)";
    let mut stmt = conn.prepare(query)?;

    let cookies = stmt
        .query_map([], |row| {
            Ok(ChromeCookie {
                name: row.get::<_, String>(0)?,
                value: row.get::<_, String>(1)?,
                encrypted_value: row.get::<_, Vec<u8>>(2)?,
                host: row.get::<_, String>(3)?,
                path: row.get::<_, String>(4)?,
                expires: row.get::<_, i64>(5)?,
                secure: row.get::<_, bool>(6)?,
                same_site: row.get::<_, i64>(7)?,
                http_only: row.get::<_, bool>(8)?,
            })
        })?
        .filter_map(|cookie| cookie.ok())
        .map(
            |ChromeCookie {
                 name,
                 value,
                 encrypted_value,
                 host,
                 path,
                 expires,
                 secure,
                 same_site,
                 http_only,
             }|
             -> color_eyre::Result<Cookie<'static>> {
                let value = if encrypted_value.is_empty() {
                    value
                } else {
                    #[cfg(not(windows))]
                    {
                        decrypt_cookie_value(encrypted_value, variant)?
                    }

                    #[cfg(windows)]
                    {
                        decrypt_cookie_value(encrypted_value, path_provider.local_state())?
                    }
                };

                Ok(CookieBuilder::new(name, value)
                    .domain(host)
                    .path(path)
                    .expires(Expiration::from(
                        OffsetDateTime::from_unix_timestamp_nanos(chrome_to_unix_timestamp_nanos(
                            expires,
                        ))
                        .expect("Invalid date"),
                    ))
                    .secure(secure)
                    .same_site(match same_site {
                        0 => SameSite::None,
                        1 => SameSite::Lax,
                        _ => SameSite::Strict,
                    })
                    .http_only(http_only)
                    .finish()
                    .into_owned())
            },
        )
        .collect::<Result<Vec<_>, _>>()?;

    Ok(cookies)
}

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
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};


use cookie::{time::OffsetDateTime, Cookie, CookieBuilder, Expiration, SameSite};
use once_cell::sync::OnceCell;
use regex::Regex;
use rusqlite::{functions::FunctionFlags, Connection};
use serde::Deserialize;
use thiserror::Error;

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

#[derive(Debug, Error)]
pub enum DecryptChromeCookieError {
    #[error("Failed to decrypt cookie value: {source}")]
    CookieValueDecrypt {
        raw_key: Box<[u8]>,
        raw_value: Box<[u8]>,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Failed to decode cookie value as UTF-8: {source}")]
    CookieValueUtf8Decode {
        #[from]
        source: std::string::FromUtf8Error,
    },

    #[error("Failed to decrypt value due to invalid length")]
    InvalidInputLength,

    #[error("Key not found in the local state")]
    KeyNotFound,

    #[error("Failed to get key: {source}")]
    GetKey {
        key_variant: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Failed to get local state: {source}")]
    LocalState {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

#[derive(Debug, Error)]
pub enum ChromeManagerError {
    #[error("Failed to open cookies database: {source}")]
    DatabaseOpen {
        path: String,
        source: rusqlite::Error,
    },

    #[error("Failed to execute SQL query: {source}")]
    SqliteQuery {
        query: String,
        source: rusqlite::Error,
    },

    #[error("Failed to decrypt cookie value: {source}")]
    CookieValueDecrypt { source: DecryptChromeCookieError },

    #[error("Failed to create SQLite function: {source}")]
    SqliteFunctionCreate { source: rusqlite::Error },
}

/// Chrome cookies manager.
pub struct ChromeManager {
    conn: Connection,
    variant: ChromeVariant,
    path_provider: PathProvider,
    key_cache: OnceCell<Vec<u8>>,
    filter: Arc<RwLock<Regex>>,
}

impl ChromeManager {
    /// Create a new instance of `ChromeManager`.
    pub fn new(
        variant: ChromeVariant,
        path_provider: PathProvider,
    ) -> Result<Self, ChromeManagerError> {
        let db_path = path_provider.cookies_database();
        let conn =
            Connection::open(&db_path).map_err(|source| ChromeManagerError::DatabaseOpen {
                path: db_path.display().to_string(),
                source,
            })?;

        let filter: Arc<RwLock<Regex>> = Arc::new(RwLock::new(Regex::new("").unwrap()));

        {
            let filter = filter.clone();
            conn.create_scalar_function("host_filter", 1, FunctionFlags::default(), move |ctx| {
                let host = &ctx.get::<String>(0)?;
                Ok(filter
                    .read()
                    .expect("Failed to read regex filter value")
                    .is_match(host))
            })
            .map_err(|source| ChromeManagerError::SqliteFunctionCreate { source })?;
        }

        Ok(Self {
            conn,
            variant,
            path_provider,
            filter,
            key_cache: OnceCell::new(),
        })
    }

    /// Create a new instance of `ChromeManager` with the default profile.
    pub fn default(variant: ChromeVariant) -> Result<Self, ChromeManagerError> {
        let path_provider = PathProvider::default_profile(variant);

        Self::new(variant, path_provider)
    }

    /// Set a filter for the cookies.
    pub fn set_filter(&self, filter: Regex) {
        *self
            .filter
            .write()
            .expect("Failed to write regex filter value") = filter;
    }

    /// Get cookies from the database.
    pub fn get_cookies(&self) -> Result<Vec<Cookie<'static>>, ChromeManagerError> {
        let query = "SELECT name, value, encrypted_value, 
                        host_key, path, expires_utc, 
                        is_secure, samesite, is_httponly
        FROM cookies
        WHERE host_filter(host_key)";

        let mut stmt =
            self.conn
                .prepare(query)
                .map_err(|source| ChromeManagerError::SqliteQuery {
                    query: query.to_string(),
                    source,
                })?;

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
            })
            .map_err(|source| ChromeManagerError::SqliteQuery {
                query: query.to_string(),
                source,
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
                 -> Result<Cookie<'static>, ChromeManagerError> {
                    let value = if encrypted_value.is_empty() {
                        value
                    } else {
                        self.decrypt_cookie_value(encrypted_value)
                            .map_err(|source| ChromeManagerError::CookieValueDecrypt { source })?
                    };

                    Ok(CookieBuilder::new(name, value)
                        .domain(host)
                        .path(path)
                        .expires(Expiration::from(
                            OffsetDateTime::from_unix_timestamp_nanos(
                                chrome_to_unix_timestamp_nanos(expires),
                            )
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

    /// Decrypt a cookie value.
    #[cfg(all(unix, not(target_os = "macos")))]
    fn decrypt_cookie_value<V: AsRef<[u8]>>(
        &self,
        encrypted_value: V,
    ) -> Result<String, DecryptChromeCookieError> {
        /// Length of the header of the encrypted value, if present.
        const HEADER_LEN: usize = 3;

        let encrypted_value = encrypted_value.as_ref();

        let key = match encrypted_value.get(..HEADER_LEN) {
            #[cfg(target_os = "linux")]
            Some(b"v11") => Some(
                self.key_cache
                    .get_or_try_init(|| linux::get_v11_key(self.variant))
                    .map_err(|source| DecryptChromeCookieError::GetKey {
                        key_variant: "v11",
                        source: source.into(),
                    })?
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
            .map_err(|source| DecryptChromeCookieError::CookieValueDecrypt {
                raw_key: key.into(),
                raw_value: encrypted_value.into(),
                source: source.into(),
            })
        } else {
            // We assume that it's not encrypted
            String::from_utf8(encrypted_value.into()).map_err(From::from)
        }
    }

    /// Decrypt a cookie value.
    #[cfg(target_os = "macos")]
    fn decrypt_cookie_value<V: AsRef<[u8]>>(
        &self,
        encrypted_value: V,
    ) -> Result<String, DecryptChromeCookieError> {
        let encrypted_value = encrypted_value.as_ref();

        /// Length of the header of the encrypted value, if present.
        const HEADER_LEN: usize = 3;

        let key = match encrypted_value.get(..HEADER_LEN) {
            Some(b"v10") => Some(
                self.key_cache
                    .get_or_try_init(|| mac::get_v10_key(self.variant))
                    .map_err(|source| DecryptChromeCookieError::GetKey {
                        key_variant: "v11",
                        source: source.into(),
                    })?,
            ),
            _ => None,
        };

        if let Some(key) = key {
            encrypted_value::decrypt_value(
                key,
                encrypted_value
                    .get(HEADER_LEN..)
                    .ok_or_else(|| DecryptChromeCookieError::InvalidInputLength)?,
            )
            .map_err(|source| DecryptChromeCookieError::CookieValueDecrypt {
                raw_key: key.as_slice().into(),
                raw_value: encrypted_value.into(),
                source: source.into(),
            })
        } else {
            // We assume that it's not encrypted
            String::from_utf8(encrypted_value.into()).map_err(From::from)
        }
    }

    #[cfg(windows)]
    fn get_local_state(&self) -> Result<LocalState, DecryptChromeCookieError> {
        use std::{fs::File, io::BufReader};
        let path = self.path_provider.local_state();

        let file =
            BufReader::new(
                File::open(path).map_err(|e| DecryptChromeCookieError::LocalState {
                    source: Box::from(e),
                })?,
            );
        let local_state = serde_json::from_reader(file).map_err(|source| {
            DecryptChromeCookieError::LocalState {
                source: Box::from(source),
            }
        })?;

        Ok(local_state)
    }

    /// Decrypt a cookie value.
    #[cfg(windows)]
    fn decrypt_cookie_value<V: AsRef<[u8]> + AsMut<[u8]>>(
        &self,
        mut encrypted_value: V,
    ) -> Result<String, DecryptChromeCookieError> {
        let encrypted_value_ref = encrypted_value.as_ref();

        /// Length of the header of the encrypted value, if present.
        const HEADER_LEN: usize = 3;

        let key = match encrypted_value_ref.get(..HEADER_LEN) {
            Some(b"v10") => Some(self.key_cache.get_or_try_init(
                || -> Result<Vec<u8>, DecryptChromeCookieError> {
                    let local_state = self.get_local_state()?;

                    let encrypted_key = windows::get_encrypted_key(&local_state)
                        .ok_or_else(|| DecryptChromeCookieError::KeyNotFound)?;
                    windows::decrypt_dpapi_encrypted_key(encrypted_key).map_err(|source| {
                        DecryptChromeCookieError::GetKey {
                            key_variant: "v10",
                            source: source.into(),
                        }
                    })
                },
            )?),
            _ => None,
        };

        if let Some(key) = key {
            encrypted_value::decrypt_value(
                key,
                encrypted_value_ref
                    .get(HEADER_LEN..)
                    .ok_or_else(|| DecryptChromeCookieError::InvalidInputLength)?,
            )
            .map_err(|source| DecryptChromeCookieError::CookieValueDecrypt {
                raw_key: key.as_slice().into(),
                raw_value: encrypted_value_ref.into(),
                source: source.into(),
            })
        } else {
            // Values seems to be always encrypted on Windows, at least with DPAPI
            // if not with AES-256-GCM
            let encrypted_value = encrypted_value.as_mut();
            let raw_value = windows::decrypt_dpapi(encrypted_value).map_err(|source| {
                DecryptChromeCookieError::CookieValueDecrypt {
                    raw_key: Vec::new().into(),
                    raw_value: encrypted_value.as_ref().into(),
                    source: source.into(),
                }
            })?;
            String::from_utf8(raw_value).map_err(From::from)
        }
    }
}

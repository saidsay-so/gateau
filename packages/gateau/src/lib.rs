//! Library to read cookies from browsers.
//!
//! It supports Firefox and Chromium-based browsers.

use std::str::FromStr;
use std::{ffi::OsString, path::Path};

use rusqlite::{Connection, OpenFlags};

use self::chrome::ChromeVariant;

pub mod chrome;
pub mod firefox;

/// Function to filter hosts.
pub type HostFilterFn = dyn FnMut(&str) -> bool + Send + Sync;

/// Represents the supported browsers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Browser {
    Firefox,
    ChromeVariant(ChromeVariant),
}

impl std::fmt::Display for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Browser::Firefox => write!(f, "Firefox"),
            Browser::ChromeVariant(ChromeVariant::Chromium) => write!(f, "Chromium"),
            Browser::ChromeVariant(ChromeVariant::Chrome) => write!(f, "Google Chrome"),
            Browser::ChromeVariant(ChromeVariant::Edge) => write!(f, "Microsoft Edge"),
        }
    }
}

impl FromStr for Browser {
    type Err = String;

    /// Parse a browser from a string.
    ///
    /// Supported browsers are:
    /// - firefox
    /// - chromium
    /// - chrome
    /// - edge
    ///
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "firefox" => Ok(Browser::Firefox),
            "chromium" => Ok(Browser::ChromeVariant(ChromeVariant::Chromium)),
            "chrome" => Ok(Browser::ChromeVariant(ChromeVariant::Chrome)),
            "edge" => Ok(Browser::ChromeVariant(ChromeVariant::Edge)),
            _ => Err(format!(
                "'{s}' is not one of the supported browsers (firefox, chromium, chrome, edge)"
            )),
        }
    }
}

/// Get a connection to the database, while bypassing the file locking if `bypass_lock` is `true`.
/// Bypassing the lock mechanism can lead to read errors if the browser is still running and writing to the database.
fn get_connection<P: AsRef<Path>>(
    db_path: P,
    bypass_lock: bool,
) -> Result<Connection, rusqlite::Error> {
    const PREFIX_LEN: usize = "file:".len() + "?immutable=1".len();

    if bypass_lock {
        let db_path = db_path.as_ref().as_os_str();
        let immutable_path_uri = {
            let mut path = OsString::with_capacity(PREFIX_LEN + db_path.len());
            path.push("file:");
            path.push(db_path);
            path.push("?immutable=1");
            path
        };

        Connection::open_with_flags(
            immutable_path_uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
    } else {
        Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    }
}

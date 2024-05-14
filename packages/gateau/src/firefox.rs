//! Firefox cookie database management.
//!
//! ### Scheme (Firefox 104+)
//!
//! ```sql
//! CREATE TABLE moz_cookies (
//!   id INTEGER PRIMARY KEY,
//!   originAttributes TEXT NOT NULL DEFAULT '',
//!   name TEXT,
//!   value TEXT,
//!   host TEXT,
//!   path TEXT,
//!   expiry INTEGER,
//!   lastAccessed INTEGER,
//!   creationTime INTEGER,
//!   isSecure INTEGER,
//!   isHttpOnly INTEGER,
//!   inBrowserElement INTEGER DEFAULT 0,
//!   sameSite INTEGER DEFAULT 0,
//!   rawSameSite INTEGER DEFAULT 0,
//!   schemeMap INTEGER DEFAULT 0,
//!   CONSTRAINT moz_uniqueid UNIQUE (
//!     name, host, path, originAttributes
//!   )
//! );
//! ```

use std::sync::{Arc, Mutex};

use cookie::time::OffsetDateTime;
use cookie::{Cookie, CookieBuilder, Expiration, SameSite};

use rusqlite::functions::FunctionFlags;
use rusqlite::Connection;

use super::get_connection;

use super::HostFilterFn;

mod paths;
pub use paths::PathProvider;

pub type Result<T, E = FirefoxManagerError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum FirefoxManagerError {
    #[error("Failed to open Firefox cookies database")]
    SqliteOpen { source: rusqlite::Error },

    #[error("Failed to create function for host filter")]
    SqliteFunctionCreate { source: rusqlite::Error },

    #[error("Failed to get cookies from Firefox database")]
    SqliteQuery { source: rusqlite::Error },
}

/// Firefox cookie database manager.
pub struct FirefoxManager {
    path_provider: paths::PathProvider,
    conn: Connection,
    filter: Arc<Mutex<Box<HostFilterFn>>>,
}

impl FirefoxManager {
    /// Create a new Firefox manager.
    pub fn new(
        path_provider: paths::PathProvider,
        filter: Box<HostFilterFn>,
        bypass_lock: bool,
    ) -> Result<Self> {
        let conn = get_connection(path_provider.cookies_database(), bypass_lock)
            .map_err(|source| FirefoxManagerError::SqliteOpen { source })?;
        let filter = Arc::from(Mutex::from(filter));

        {
            let filter = filter.clone();
            conn.create_scalar_function("host_filter", 1, FunctionFlags::default(), move |ctx| {
                let mut f = filter.lock().expect("Failed to lock filter");
                let host = ctx.get::<String>(0)?;
                Ok(f(&host) as i64)
            })
            .map_err(|source| FirefoxManagerError::SqliteFunctionCreate { source })?;
        }

        Ok(Self {
            path_provider,
            conn,
            filter,
        })
    }

    /// Get the path provider.
    pub fn path_provider(&self) -> &paths::PathProvider {
        &self.path_provider
    }

    /// Create a new Firefox manager with the default profile.
    pub fn default_profile(filter: Box<HostFilterFn>, bypass_lock: bool) -> Result<Self> {
        let path_provider = paths::PathProvider::default_profile();
        Self::new(path_provider, filter, bypass_lock)
    }

    /// Get all cookies from the database.
    ///
    /// ## Limitations
    ///
    /// The expiry time is clamped to the maximum UNIX timestamp value supported by the underlying
    /// library (253402300799), despite the fact that Firefox uses a 64-bit integer to store the expiry
    /// time.
    pub fn get_cookies(&self) -> Result<Vec<Cookie<'static>>> {
        let query = "SELECT name, value, host, path, 
                        expiry, isSecure, sameSite, 
                        isHttpOnly
        FROM moz_cookies
        WHERE host_filter(host)";

        let mut stmt = self
            .conn
            .prepare(query)
            .map_err(|source| FirefoxManagerError::SqliteQuery { source })?;

        let cookies = stmt
            .query_map([], |row| {
                Ok(
                    CookieBuilder::new(row.get::<_, String>(0)?, row.get::<_, String>(1)?)
                        .domain(row.get::<_, String>(2)?)
                        .path(row.get::<_, String>(3)?)
                        .expires(Expiration::from(
                            OffsetDateTime::from_unix_timestamp(row.get(4)?)
                                .expect("Invalid timestamp"),
                        ))
                        .secure(row.get::<_, isize>(5)? != 0)
                        .same_site(match row.get(6)? {
                            0 => SameSite::None,
                            1 => SameSite::Lax,
                            _ => SameSite::Strict,
                        })
                        .http_only(row.get::<_, isize>(7)? != 0)
                        .into(),
                )
            })
            .map_err(|source| FirefoxManagerError::SqliteQuery { source })?
            .filter_map(|c| c.ok())
            .collect::<Vec<_>>();

        Ok(cookies)
    }
}

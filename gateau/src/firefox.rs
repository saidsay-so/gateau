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

use color_eyre::Result;
use cookie::time::OffsetDateTime;
use cookie::{Cookie, CookieBuilder, Expiration, SameSite};

use rusqlite::Connection;

pub(crate) mod paths;

/// Get all cookies from the database.
pub fn get_cookies(conn: &Connection) -> Result<Vec<Cookie<'static>>> {
    let query = "SELECT name, value, host, path, 
                        expiry, isSecure, sameSite, 
                        isHttpOnly
        FROM moz_cookies
        WHERE host_filter(host)";

    let mut stmt = conn.prepare(query)?;

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
                    .finish()
                    .into_owned(),
            )
        })?
        .filter_map(|c| c.ok())
        .collect::<Vec<_>>();

    Ok(cookies)
}

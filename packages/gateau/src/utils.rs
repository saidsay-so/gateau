use std::{ffi::OsString, path::Path};

use rusqlite::{Connection, OpenFlags};

/// Get a connection to the database, while bypassing the file locking if `bypass_lock` is `true`.
pub fn get_connection<P: AsRef<Path>>(
    db_path: P,
    bypass_lock: bool,
) -> Result<Connection, rusqlite::Error> {
    if bypass_lock {
        let db_path = db_path.as_ref().as_os_str();
        let immutable_path_uri = {
            let mut path = OsString::with_capacity(17 + db_path.len());
            path.push("file:");
            path.push(db_path);
            path.push("?immutable=1");
            path
        };

        // This can lead to read errors if the browser is still running and writing to the database.
        Connection::open_with_flags(
            immutable_path_uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
    } else {
        Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    }
}

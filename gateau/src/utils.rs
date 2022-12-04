use std::path::Path;

use rusqlite::{Connection, OpenFlags};

/// Get a connection to the database, while bypassing the file locking if `bypass_lock` is `true`.
pub fn get_connection<P: AsRef<Path>>(
    db_path: P,
    bypass_lock: bool,
) -> color_eyre::Result<Connection> {
    let connection = if bypass_lock {
        // This can lead to read errors if the browser is still running and writing to the database.
        let immutable_path_uri = format!("file:{}?immutable=1", db_path.as_ref().display());
        Connection::open_with_flags(
            immutable_path_uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
    } else {
        Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    }?;

    Ok(connection)
}

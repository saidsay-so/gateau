use std::{ffi::OsString, path::Path};

use rusqlite::{functions::FunctionFlags, Connection, OpenFlags};

/// Get a connection to the database, while bypassing the file locking if `bypass_lock` is `true`.
pub fn get_connection<P: AsRef<Path>>(
    db_path: P,
    bypass_lock: bool,
) -> color_eyre::Result<Connection> {
    let conn = if bypass_lock {
        // This can lead to read errors if the browser is still running and writing to the database.
        let db_path = db_path.as_ref().as_os_str();
        let immutable_path_uri = {
            let mut path = OsString::with_capacity(17 + db_path.len());
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
    }?;

    Ok(conn)
}

pub(crate) fn sqlite_predicate_builder<F, S: AsRef<str>>(
    conn: &Connection,
    name: S,
    predicate: F,
) -> color_eyre::Result<()>
where
    F: Fn(&str) -> bool + std::marker::Sync + std::marker::Send + std::panic::UnwindSafe + 'static,
{
    Ok(
        conn.create_scalar_function(name.as_ref(), 1, FunctionFlags::default(), move |ctx| {
            Ok(predicate(&ctx.get::<String>(0)?))
        })?,
    )
}

//! The node library of the toolchain prototype. The host calls it through the C ABI.

use libsqlite3_sys as sql;

/// Calls the C code of the local crate `cmix`.
#[unsafe(no_mangle)]
pub extern "C" fn patina_fixture_mix(a: i32, b: i32) -> i32 {
    cmix::mix(a, b)
}

/// Runs `select 6 * 7` in an in-memory SQLite database. Returns -1 on an error.
#[unsafe(no_mangle)]
pub extern "C" fn patina_fixture_sqlite() -> i32 {
    let mut db = std::ptr::null_mut();
    let mut stmt = std::ptr::null_mut();
    let mut value = -1;
    // SAFETY: the strings end with a zero, the out pointers are valid, and each handle closes.
    unsafe {
        if sql::sqlite3_open(c":memory:".as_ptr(), &mut db) == sql::SQLITE_OK
            && sql::sqlite3_prepare_v2(db, c"select 6 * 7".as_ptr(), -1, &mut stmt, std::ptr::null_mut())
                == sql::SQLITE_OK
        {
            if sql::sqlite3_step(stmt) == sql::SQLITE_ROW {
                value = sql::sqlite3_column_int(stmt, 0);
            }
            sql::sqlite3_finalize(stmt);
        }
        sql::sqlite3_close(db);
    }
    value
}

/// Adds 1 to 4 on four threads of the Rust standard library.
#[unsafe(no_mangle)]
pub extern "C" fn patina_fixture_threads() -> i32 {
    let threads: Vec<_> = (1..=4).map(|i| std::thread::spawn(move || i)).collect();
    threads.into_iter().map(|t| t.join().unwrap_or(0)).sum()
}

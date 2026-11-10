use super::*;
use ibex2::stdlib::sqlite::{Database, Provider};
fn open(dir: &tempfile::TempDir) -> Database {
    let path = dir.path().canonicalize().unwrap().join("app.db");
    Database::new(SqliteProvider.open(Location { path }).unwrap())
}
#[test]
fn persists_typed_values_and_prepared_bindings() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    db.execute(
        "CREATE TABLE items (id INTEGER, name TEXT, data BLOB, nothing_value, real_value REAL)",
        &[],
    )
    .unwrap();
    let statement = db
        .prepare("INSERT INTO items VALUES (?1,?2,?3,?4,?5)")
        .unwrap();
    assert_eq!(statement.info.parameter_count, 5);
    let params = [
        Value::Integer(i64::MAX),
        Value::Text("'); DROP TABLE items; --".into()),
        Value::Blob(vec![0, 255]),
        Value::Null,
        Value::Real(1.5),
    ];
    assert_eq!(statement.execute(&params).unwrap().changes, 1);
    statement.close();
    assert!(statement.execute(&params).is_err());
    db.close().unwrap();
    let db = open(&dir);
    let result = db.query("SELECT * FROM items", &[]).unwrap();
    assert_eq!(result.rows, vec![params.to_vec()]);
    assert_eq!(
        db.query("SELECT id AS x,id AS x FROM items", &[])
            .unwrap()
            .columns,
        ["x", "x"]
    );
    let statement = db.prepare("SELECT * FROM items").unwrap();
    db.close().unwrap();
    db.close().unwrap();
    assert!(statement.query(&[]).is_err());
}
#[test]
fn transactions_roll_back_and_deny_manual_control() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    db.execute("CREATE TABLE items(id INTEGER UNIQUE)", &[])
        .unwrap();
    let batch = vec![
        Command {
            sql: "INSERT INTO items VALUES (?)".into(),
            params: vec![Value::Integer(1)]
        };
        2
    ];
    assert!(db.transaction(&batch).is_err());
    assert_eq!(
        db.query("SELECT COUNT(*) FROM items", &[]).unwrap().rows[0][0],
        Value::Integer(0)
    );
    assert_eq!(db.transaction(&batch[..1]).unwrap()[0].changes, 1);
    for sql in ["BEGIN", "COMMIT", "SAVEPOINT x", "ROLLBACK"] {
        assert!(db.execute(sql, &[]).is_err(), "{sql}");
    }
}
#[test]
fn native_authorizer_prevents_sql_filesystem_escapes() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    for sql in [
        "ATTACH DATABASE '/tmp/ibex-escape.db' AS escaped",
        "DETACH main",
        "VACUUM INTO '/tmp/ibex-escape.db'",
        "PRAGMA temp_store_directory='/tmp'",
        "PRAGMA writable_schema=ON",
        "PRAGMA journal_mode=OFF",
        "SELECT load_extension('/tmp/evil.so')",
        "CREATE VIRTUAL TABLE x USING fts5(body)",
    ] {
        assert!(
            db.prepare(sql).is_err() || db.execute(sql, &[]).is_err(),
            "{sql}"
        );
    }
    assert!(db.prepare("SELECT 1; SELECT 2").is_err());
    assert!(db.prepare("").is_err());
    assert!(db.execute("PRAGMA user_version=3", &[]).is_ok());
    assert!(db.execute("SELECT 1", &[]).is_err());
    assert!(db.query("CREATE TABLE bad(id)", &[]).is_err());
}
#[test]
fn concurrent_transactions_never_interleave() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    db.execute("CREATE TABLE counter(value INTEGER)", &[])
        .unwrap();
    db.execute("INSERT INTO counter VALUES (0)", &[]).unwrap();
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let db = db.clone();
            std::thread::spawn(move || {
                for _ in 0..20 {
                    db.transaction(&[
                        Command {
                            sql: "UPDATE counter SET value=value+1".into(),
                            params: vec![],
                        },
                        Command {
                            sql: "UPDATE counter SET value=value+1".into(),
                            params: vec![],
                        },
                    ])
                    .unwrap();
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(
        db.query("SELECT value FROM counter", &[]).unwrap().rows[0][0],
        Value::Integer(160)
    );
}
#[test]
fn separate_connections_use_sqlite_locking() {
    let dir = tempfile::tempdir().unwrap();
    let first = open(&dir);
    first
        .execute("CREATE TABLE items(value INTEGER)", &[])
        .unwrap();
    let second = open(&dir);
    let worker = std::thread::spawn(move || {
        for _ in 0..20 {
            second.execute("INSERT INTO items VALUES (2)", &[]).unwrap();
        }
    });
    for _ in 0..20 {
        first.execute("INSERT INTO items VALUES (1)", &[]).unwrap();
    }
    worker.join().unwrap();
    assert_eq!(
        first.query("SELECT COUNT(*) FROM items", &[]).unwrap().rows[0][0],
        Value::Integer(40)
    );
}
#[test]
fn bounds_results_and_validates_parameters() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    assert!(db.query("SELECT ?", &[]).is_err());
    assert!(db.query("SELECT ?", &[Value::Real(f64::NAN)]).is_err());
    assert!(db.query("SELECT zeroblob(16777216)", &[]).is_err());
    assert!(db.query("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<100001) SELECT x FROM n", &[]).is_err());
}
#[cfg(unix)]
#[test]
fn rejects_database_and_parent_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("linked")).unwrap();
    assert!(SqliteProvider
        .open(Location {
            path: root.join("linked/app.db")
        })
        .is_err());
    std::os::unix::fs::symlink(outside.path().join("outside.db"), root.join("app.db")).unwrap();
    assert!(SqliteProvider
        .open(Location {
            path: root.join("app.db")
        })
        .is_err());
    assert!(!outside.path().join("outside.db").exists());
}

#[cfg(unix)]
#[test]
fn journal_symlinks_cannot_overwrite_other_files() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    db.execute("CREATE TABLE items(value INTEGER)", &[])
        .unwrap();
    let outside = dir.path().join("outside");
    std::fs::write(&outside, b"untouched").unwrap();
    std::os::unix::fs::symlink(&outside, dir.path().join("app.db-journal")).unwrap();
    assert!(db.execute("INSERT INTO items VALUES (1)", &[]).is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), b"untouched");
}

#[cfg(windows)]
#[test]
fn reparse_database_parents_and_sidecars_are_refused_before_open() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), b"private").unwrap();
    for (index, suffix) in ["", "-journal", "-wal", "-shm"].iter().enumerate() {
        let link = root.path().join(format!("db{index}{suffix}"));
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:IBEX_SQLITE_TEST_LINK -Value $env:IBEX_SQLITE_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("IBEX_SQLITE_TEST_LINK", &link)
            .env("IBEX_SQLITE_TEST_TARGET", outside.path())
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(SqliteProvider
            .open(Location {
                path: root.path().join(format!("db{index}"))
            })
            .is_err());
    }
    assert!(SqliteProvider
        .open(Location {
            path: root.path().join("db0/nested.db")
        })
        .is_err());
    assert_eq!(
        std::fs::read(outside.path().join("secret")).unwrap(),
        b"private"
    );
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 1);
}
#[test]
fn a_refused_statement_names_what_was_refused_and_the_rule() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    for (sql, said) in [
        (
            "ATTACH DATABASE '/tmp/ibex-escape.db' AS escaped",
            "ATTACH and DETACH",
        ),
        ("PRAGMA journal_mode=OFF", "PRAGMA journal_mode: "),
        ("SELECT load_extension('/tmp/evil.so')", "load_extension: "),
        (
            "CREATE VIRTUAL TABLE x USING fts5(body)",
            "a virtual table: ",
        ),
        ("SAVEPOINT x", "SAVEPOINT: use transaction([...])"),
        (
            "BEGIN",
            "BEGIN, COMMIT and ROLLBACK: use transaction([...])",
        ),
    ] {
        let got = format!("{:?}", db.execute(sql, &[]).unwrap_err());
        assert!(
            got.contains(&format!("not authorized: {said}")),
            "{sql}: {got}"
        );
    }
    // A table-valued PRAGMA prepares its PRAGMA while the query steps.
    let got = format!(
        "{:?}",
        db.query("SELECT * FROM pragma_foreign_keys", &[])
            .unwrap_err()
    );
    assert!(
        got.contains("not authorized: PRAGMA foreign_keys: "),
        "{got}"
    );
    // Refused while it steps: `VACUUM INTO` attaches its target then.
    let got = format!(
        "{:?}",
        db.execute("VACUUM INTO 'copy.db'", &[]).unwrap_err()
    );
    assert!(got.contains("not authorized: ATTACH and DETACH"), "{got}");
    // A batch names the statement by its place and its SQL, and runs none.
    let batch = [
        Command {
            sql: "CREATE TABLE IF NOT EXISTS a (x)".into(),
            params: vec![],
        },
        Command {
            sql: "PRAGMA foreign_keys=OFF".into(),
            params: vec![],
        },
    ];
    let got = format!("{:?}", db.transaction(&batch).unwrap_err());
    assert!(
        got.contains("SQLite: statement 2 of 2 (PRAGMA foreign_keys=OFF): not authorized: PRAGMA foreign_keys: ")
            && got.contains("PRAGMA user_version, application_id"),
        "{got}"
    );
    let tables = db
        .query("SELECT COUNT(*) FROM sqlite_schema WHERE name = 'a'", &[])
        .unwrap();
    assert_eq!(tables.rows[0][0], Value::Integer(0));
    // Any failure in a batch names its statement.
    let missing = Command {
        sql: "INSERT INTO missing VALUES (1)".into(),
        params: vec![],
    };
    let got = format!(
        "{:?}",
        db.transaction(&[batch[0].clone(), missing]).unwrap_err()
    );
    assert!(
        got.contains("statement 2 of 2 (INSERT INTO missing VALUES (1)): no such table"),
        "{got}"
    );
}
#[test]
fn only_sqlite_s_own_result_marks_a_failure_busy() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    db.execute("CREATE TABLE words(w TEXT NOT NULL UNIQUE)", &[])
        .unwrap();
    // The app's words come after the provider's prefix, never in it.
    let words = "busy, database is locked, denied: ";
    let insert = Command {
        sql: format!("INSERT INTO words VALUES ('{words}') /* {words} */"),
        params: vec![],
    };
    let batch = vec![insert; 2];
    let got = format!("{:?}", db.transaction(&batch).unwrap_err());
    assert!(
        got.starts_with("Failed(\"SQLite: statement 2 of 2 (INSERT INTO words"),
        "{got}"
    );
    // Another connection writing: SQLite's own busy, after its 5 s wait.
    let path = dir.path().canonicalize().unwrap().join("app.db");
    let held = rusqlite::Connection::open(path).unwrap();
    held.execute_batch("BEGIN IMMEDIATE").unwrap();
    let got = format!("{:?}", db.execute("INSERT INTO words VALUES ('x')", &[]));
    held.execute_batch("ROLLBACK").unwrap();
    assert!(got.starts_with("Err(Failed(\"SQLite EBUSY: "), "{got}");
}

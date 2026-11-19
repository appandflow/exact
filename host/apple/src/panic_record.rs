//! A panic's message, location and thread, written to a file before the
//! process aborts. A release build aborts on panic (`panic = "abort"`), and
//! the SIGABRT that follows carries none of them. The Observe module's launch
//! part names the file (`ExactLaunchContext.recordPanics`) and reads it at the
//! next launch beside its fatal-signal record. A Rust module loaded as a
//! dylib has its own `std` and panic hook; its hook writes the same record
//! through `exact_logic_abi::panics`.

use std::path::PathBuf;
use std::sync::OnceLock;

static PATH: OnceLock<PathBuf> = OnceLock::new();

/// The file's text: `{"message", "location", "thread"}`, `location` as `file:line`.
pub fn record(message: &str, location: Option<String>, thread: Option<&str>) -> String {
    serde_json::json!({ "message": message, "location": location, "thread": thread }).to_string()
}

/// The panic payload as text: a `&str` or a `String`, as `panic!` makes them.
fn message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_default()
}

/// Installs a hook that writes [`record`] to `path`, then runs the hook it
/// replaced. Only the first call installs it. A panic that unwinds and is
/// caught also writes the file.
pub fn install(path: PathBuf) {
    if PATH.set(path).is_err() {
        return;
    }
    exact_logic_abi::panics::report_to(|message, file, line| {
        write(
            message,
            (!file.is_empty()).then(|| format!("{file}:{line}")),
        )
    });
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()));
        write(&message(info.payload()), location);
        previous(info);
    }));
}

fn write(message: &str, location: Option<String>) {
    if let Some(path) = PATH.get() {
        let thread = std::thread::current();
        let _ = std::fs::write(path, record(message, location, thread.name()));
    }
}

/// The C export the launch part calls (`include/exact.h`).
#[macro_export]
macro_rules! panic_record_exports {
    () => {
        /// Write each later panic's message, location and thread to `path`.
        /// # Safety
        /// `path` holds `len` readable bytes of UTF-8.
        #[no_mangle]
        pub unsafe extern "C" fn exact_panic_record(path: *const u8, len: usize) {
            if path.is_null() {
                return;
            }
            // SAFETY: the caller supplies len readable bytes.
            let bytes = unsafe { ::std::slice::from_raw_parts(path, len) };
            if let Ok(path) = ::std::str::from_utf8(bytes) {
                $crate::panic_record::install(path.into());
            }
        }
    };
}

#[cfg(test)]
mod tests {
    // The hook is installed only in a child process, so it is never swapped
    // under concurrently running tests.
    use super::*;

    #[test]
    fn a_panic_writes_its_record_and_runs_the_previous_hook() {
        const TEST: &str =
            "panic_record::tests::a_panic_writes_its_record_and_runs_the_previous_hook";
        if let Some(path) = std::env::var_os("EXACT_PANIC_RECORD_CHILD") {
            install(path.into());
            std::thread::Builder::new()
                .name("worker".into())
                .spawn(|| panic!("generated {} levels", 3))
                .unwrap()
                .join()
                .unwrap_err();
            return;
        }
        let dir = std::env::temp_dir().join(format!("exact-panic-record-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("panic.json");
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env("EXACT_PANIC_RECORD_CHILD", &path)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{stderr}");
        // The default hook it replaced still reported the panic.
        assert!(stderr.contains("generated 3 levels"), "{stderr}");
        let o: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(o["message"], "generated 3 levels");
        assert_eq!(o["thread"], "worker");
        assert!(
            o["location"]
                .as_str()
                .unwrap()
                .starts_with(&format!("{}:", file!())),
            "{o}"
        );
    }

    // A Rust module loaded as a dylib reports its panic through
    // `exact_logic_abi::panics::report`, on the thread that panicked.
    #[test]
    #[allow(unsafe_code)]
    fn a_loaded_modules_panic_writes_its_record() {
        const TEST: &str = "panic_record::tests::a_loaded_modules_panic_writes_its_record";
        if let Some(path) = std::env::var_os("EXACT_MODULE_PANIC_RECORD_CHILD") {
            install(path.into());
            std::thread::Builder::new()
                .name("session".into())
                .spawn(|| {
                    let (message, file) = ("module refused", "logic/src/lib.rs");
                    // SAFETY: both strings outlive the call.
                    unsafe {
                        exact_logic_abi::panics::report(
                            message.as_ptr(),
                            message.len(),
                            file.as_ptr(),
                            file.len(),
                            12,
                        )
                    }
                })
                .unwrap()
                .join()
                .unwrap();
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("exact-module-panic-record-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("panic.json");
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env("EXACT_MODULE_PANIC_RECORD_CHILD", &path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let o: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            o,
            serde_json::json!({
                "message": "module refused", "location": "logic/src/lib.rs:12", "thread": "session",
            })
        );
    }

    #[test]
    fn a_record_names_the_message_location_and_thread() {
        let text = record(
            "index out of bounds",
            Some("data/src/lib.rs:42".into()),
            Some("main"),
        );
        let o: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            o,
            serde_json::json!({
                "message": "index out of bounds", "location": "data/src/lib.rs:42", "thread": "main",
            })
        );
    }

    #[test]
    fn an_unnamed_thread_and_an_unknown_location_are_null() {
        let o: serde_json::Value = serde_json::from_str(&record("", None, None)).unwrap();
        assert_eq!(o["location"], serde_json::Value::Null);
        assert_eq!(o["thread"], serde_json::Value::Null);
    }

    #[test]
    fn a_payload_is_read_as_a_str_or_a_string() {
        let literal: Box<dyn std::any::Any + Send> = Box::new("literal");
        let formatted: Box<dyn std::any::Any + Send> = Box::new(format!("at {}", 3));
        let other: Box<dyn std::any::Any + Send> = Box::new(7_u8);
        assert_eq!(message(literal.as_ref()), "literal");
        assert_eq!(message(formatted.as_ref()), "at 3");
        assert_eq!(message(other.as_ref()), "");
    }
}

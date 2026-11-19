//! A native module's panics, reported through the host that loaded it. A
//! module loaded as a dylib (`logic/src/native.rs`) links its own `std`, with
//! its own panic hook, so a hook the host installs never sees its panics. The
//! loader hands each module [`report`] (`exact_logic_forward_panics`), and the
//! module's hook calls it before the panic aborts.

use std::sync::{Once, OnceLock};

/// Called from a module's panic hook with the panic's message, file and line.
pub type Report = unsafe extern "C" fn(
    message: *const u8,
    message_len: usize,
    file: *const u8,
    file_len: usize,
    line: u32,
);

static HOST: OnceLock<fn(&str, &str, u32)> = OnceLock::new();

/// Send each later panic in a loaded module to `report`, with its message,
/// file and line; the file is empty when the panic has no location. Only the
/// first call sets it.
pub fn report_to(report: fn(message: &str, file: &str, line: u32)) {
    let _ = HOST.set(report);
}

/// The host's [`Report`]: [`report_to`]'s function, when one is set. It runs
/// on the thread that panicked, so the host's `std` names that thread.
/// # Safety
/// `message` and `file` hold `message_len` and `file_len` readable bytes.
pub unsafe extern "C" fn report(
    message: *const u8,
    message_len: usize,
    file: *const u8,
    file_len: usize,
    line: u32,
) {
    unsafe fn text<'a>(ptr: *const u8, len: usize) -> &'a str {
        if ptr.is_null() {
            return "";
        }
        // SAFETY: the caller supplies len readable bytes.
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(ptr, len) }).unwrap_or_default()
    }
    if let Some(report) = HOST.get() {
        // SAFETY: as this function's own contract.
        unsafe { report(text(message, message_len), text(file, file_len), line) };
    }
}

/// In the module: install, once, a hook that hands each panic to `report`
/// and then runs the hook it replaced.
pub fn forward(report: Report) {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info.payload_as_str().unwrap_or_default();
            let (file, line) = info.location().map_or(("", 0), |l| (l.file(), l.line()));
            // SAFETY: both strings outlive the call.
            unsafe {
                report(
                    message.as_ptr(),
                    message.len(),
                    file.as_ptr(),
                    file.len(),
                    line,
                )
            };
            previous(info);
        }));
    });
}

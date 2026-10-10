//! One running copy per data directory, held by a file lock.
//!
//! The single-instance plugin hands a second launch over through a socket in
//! `/tmp`, which the macOS App Sandbox denies: there the plugin gives up and
//! lets every launch run. The login agent starts the executable directly, so
//! without this a second full copy came up beside the running one, with its
//! own tray icon and a window stacked exactly on the first.

use std::{
    fs::{File, OpenOptions, TryLockError},
    io,
    path::Path,
    sync::OnceLock,
    thread,
    time::{Duration, Instant},
};

const LOCK_FILE_NAME: &str = "instance.lock";
/// How long a launch waits for a held lock. A restart starts the new copy
/// just before the old one exits, and that copy must outlast the overlap.
const HANDOVER_GRACE: Duration = Duration::from_secs(1);
const HANDOVER_POLL: Duration = Duration::from_millis(50);

/// Keeps the claimed lock for the life of the process; the OS drops it when
/// the process ends, however it ends.
static CLAIMED: OnceLock<File> = OnceLock::new();

/// Claims `data_dir` for this process. `Ok(false)` means another running copy
/// already holds it and this one should leave.
pub fn claim_single_instance(data_dir: &Path) -> io::Result<bool> {
    let deadline = Instant::now() + HANDOVER_GRACE;
    let lock = loop {
        if let Some(lock) = try_lock(data_dir)? {
            break lock;
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(HANDOVER_POLL);
    };
    // A second claim by this process keeps the first lock, which is the claim.
    let _ = CLAIMED.set(lock);
    Ok(true)
}

/// Asks the copy that holds the claim to show its window. A launch that is
/// turned away calls this first, unless it was a login launch. Only macOS
/// needs it: elsewhere the single-instance plugin hands the launch over.
pub fn request_show(signal_name: &str) {
    #[cfg(target_os = "macos")]
    signal::post(signal_name);
    #[cfg(not(target_os = "macos"))]
    let _ = signal_name;
}

/// Runs `handler` on the main thread each time another copy calls
/// [`request_show`] with the same name. The first handler stays for the life
/// of the process.
pub fn on_show_request(signal_name: &str, handler: impl Fn() + Send + Sync + 'static) {
    #[cfg(target_os = "macos")]
    signal::observe(signal_name, Box::new(handler));
    #[cfg(not(target_os = "macos"))]
    let _ = (signal_name, handler);
}

#[cfg(target_os = "macos")]
mod signal {
    use std::{ffi::CString, sync::OnceLock};

    use libc::c_char;

    type Handler = Box<dyn Fn() + Send + Sync>;

    static HANDLER: OnceLock<Handler> = OnceLock::new();

    // Implemented in `native/macos_instance_signal.m`, which this crate's
    // `build.rs` compiles and links. Both borrow `name` for the call only, and
    // the callback is invoked on the main queue with no arguments.
    // SAFETY: calling these functions only requires that contract.
    unsafe extern "C" {
        fn voya_macos_post_signal(name: *const c_char);
        fn voya_macos_observe_signal(name: *const c_char, callback: extern "C" fn()) -> bool;
    }

    extern "C" fn dispatch() {
        if let Some(handler) = HANDLER.get() {
            handler();
        }
    }

    pub(super) fn post(name: &str) {
        let Ok(name) = CString::new(name) else {
            return;
        };
        // SAFETY: `name` is a live NUL-terminated string for the whole call.
        unsafe { voya_macos_post_signal(name.as_ptr()) };
    }

    pub(super) fn observe(name: &str, handler: Handler) {
        let Ok(name) = CString::new(name) else {
            return;
        };
        if HANDLER.set(handler).is_err() {
            return;
        }
        // SAFETY: `name` is a live NUL-terminated string for the whole call,
        // and `dispatch` is a plain function that lives as long as the process.
        if !unsafe { voya_macos_observe_signal(name.as_ptr(), dispatch) } {
            tracing::warn!("could not listen for show requests from other launches");
        }
    }
}

fn try_lock(data_dir: &Path) -> io::Result<Option<File>> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(data_dir.join(LOCK_FILE_NAME))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_lock_turns_the_next_claim_away_until_it_is_released() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let first = try_lock(dir.path())?;
        assert!(first.is_some());
        assert!(try_lock(dir.path())?.is_none());
        drop(first);
        assert!(try_lock(dir.path())?.is_some());
        Ok(())
    }
}

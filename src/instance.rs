//! One copy of Acervus per database. Two copies on the same database would
//! both run the sync worker, the outbox and the daily backup, so a second
//! start brings the open window forward and exits without touching the data
//! folder, the database or the log.

use std::path::Path;

/// Name of the lock for the real app's database.
const MAIN_LOCK: &str = "Local\\AcervusFinance-Main";

/// The lock's name. `Local\` keeps it per Windows session. A custom database
/// (`ACERVUS_DB`: demos, development) gets a name of its own, so it can run
/// beside the real app, but two copies on one database cannot.
pub fn lock_name(custom_db: Option<&Path>) -> String {
    let Some(path) = custom_db else {
        return MAIN_LOCK.to_string();
    };
    // Windows paths ignore letter case and take either slash. A hash keeps the
    // name short and free of backslashes, which a lock name may not hold after
    // `Local\`. FNV-1a, written out, because the name must be the same in every
    // run and every build.
    let spelled = path.to_string_lossy().to_lowercase().replace('/', "\\");
    let hash = spelled.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    format!("Local\\AcervusFinance-Db-{hash:016x}")
}

/// Holds the lock for the life of the process. The system frees it when the
/// process ends or crashes, so a lock is never left behind.
pub struct Instance {
    #[cfg(windows)]
    _handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl Drop for Instance {
    fn drop(&mut self) {
        if !self._handle.is_invalid() {
            // SAFETY: the handle was made by `claim` and is closed only here.
            let _ = unsafe { windows::Win32::Foundation::CloseHandle(self._handle) };
        }
    }
}

#[cfg(windows)]
mod claim_windows {
    use super::Instance;
    use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, IsIconic, SW_RESTORE, SetForegroundWindow, ShowWindow};
    use windows::core::{HSTRING, w};

    /// Claims the lock. `None` when another copy already holds it. If the
    /// system can't make the lock for another reason, the start goes ahead as
    /// it always did: a broken lock must never keep the app from opening.
    pub fn claim(name: &str) -> Option<Instance> {
        // SAFETY: plain Win32 calls with a valid, NUL-terminated name.
        unsafe {
            let handle = match CreateMutexW(None, false, &HSTRING::from(name)) {
                Ok(handle) => handle,
                Err(e) => {
                    log::warn!("could not make the single-copy lock, starting anyway: {e}");
                    return Some(Instance { _handle: Default::default() });
                }
            };
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return None;
            }
            Some(Instance { _handle: handle })
        }
    }

    /// Brings the window of the copy that is already open to the front. Only
    /// asks the system; it never sends clicks or keys to anyone. GPUI's window
    /// class is shared by every GPUI app, so the title must match too.
    pub fn show_open_copy() {
        // SAFETY: plain Win32 calls; the window handle comes from the system.
        unsafe {
            let Ok(window) = FindWindowW(w!("Zed::Window"), &HSTRING::from(crate::APP_NAME)) else {
                return;
            };
            if IsIconic(window).as_bool() {
                let _ = ShowWindow(window, SW_RESTORE);
            }
            let _ = SetForegroundWindow(window);
        }
    }
}

#[cfg(windows)]
pub use claim_windows::{claim, show_open_copy};

#[cfg(not(windows))]
pub fn claim(_name: &str) -> Option<Instance> {
    Some(Instance {})
}

#[cfg(not(windows))]
pub fn show_open_copy() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn unique(tag: &str) -> String {
        format!("Local\\AcervusTest-{}-{tag}", std::process::id())
    }

    #[test]
    fn the_normal_lock_name_is_fixed_and_in_the_session_namespace() {
        let name = lock_name(None);
        assert_eq!(name, lock_name(None));
        assert!(name.starts_with("Local\\"));
        assert!(!name["Local\\".len()..].contains('\\'));
    }

    #[test]
    fn a_custom_database_gets_its_own_name() {
        let name = lock_name(Some(&PathBuf::from(r"C:\Demo\demo.db")));
        assert_ne!(name, lock_name(None));
        assert!(name.starts_with("Local\\"));
        assert!(!name["Local\\".len()..].contains('\\'));
        assert!(name.len() < 200);
    }

    #[test]
    fn the_same_database_gives_the_same_name_whatever_the_spelling() {
        let a = lock_name(Some(&PathBuf::from(r"C:\Demo\demo.db")));
        let b = lock_name(Some(&PathBuf::from("c:/demo/DEMO.db")));
        assert_eq!(a, b);
    }

    #[test]
    fn different_databases_give_different_names() {
        let a = lock_name(Some(&PathBuf::from(r"C:\Demo\one.db")));
        let b = lock_name(Some(&PathBuf::from(r"C:\Demo\two.db")));
        assert_ne!(a, b);
    }

    #[cfg(windows)]
    #[test]
    fn a_name_can_be_claimed_once_until_the_first_claim_is_dropped() {
        let name = unique("claim");
        let first = claim(&name);
        assert!(first.is_some());
        assert!(claim(&name).is_none());
        drop(first);
        assert!(claim(&name).is_some());
    }
}

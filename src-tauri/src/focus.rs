//! focus.rs — capture and restore the app holding the user's selection.
//!
//! Quill's popup must be focusable (the refine chat needs keyboard input), so
//! showing it steals focus from the source app. Accept therefore runs:
//! hide popup → `activate(source)` → short settle → clipboard paste.
//! Without the restore, the synthetic Cmd/Ctrl+V lands in Quill, not the
//! user's text. Handy avoids this entirely with a non-activating panel, but
//! that cannot receive typed input (Handy `overlay.rs`, tauri-nspanel).

/// Opaque handle to the previously-frontmost application.
#[derive(Debug, Clone, Copy)]
pub struct SourceApp {
    #[cfg(target_os = "macos")]
    pub pid: i32,
    #[cfg(target_os = "windows")]
    pub hwnd: isize,
    /// Fallback for platforms with no implementation yet.
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    pub _unsupported: (),
}

/// Remember which app is frontmost. Call BEFORE showing the popup.
pub fn capture_frontmost() -> Option<SourceApp> {
    #[cfg(target_os = "macos")]
    {
        macos::capture().map(|pid| SourceApp { pid })
    }
    #[cfg(target_os = "windows")]
    {
        windows_impl::capture().map(|hwnd| SourceApp { hwnd })
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// Re-activate the captured app so the subsequent paste chord lands there.
pub fn activate(source: SourceApp) {
    #[cfg(target_os = "macos")]
    macos::activate(source.pid);
    #[cfg(target_os = "windows")]
    windows_impl::activate(source.hwnd);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = source;
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

    pub fn capture() -> Option<i32> {
        let workspace = NSWorkspace::sharedWorkspace();
        let app = workspace.frontmostApplication()?;
        Some(app.processIdentifier())
    }

    pub fn activate(pid: i32) {
        if pid <= 0 {
            return;
        }
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        else {
            log::warn!("source app pid {pid} no longer running");
            return;
        };
        // ActivateIgnoringOtherApps: bring it forward even though Quill is
        // currently key. Deprecated-in-favor-of variants exist on macOS 14+,
        // but activateWithOptions remains available and is the portable call.
        #[allow(deprecated)]
        let ok = app.activateWithOptions(
            NSApplicationActivationOptions::ActivateIgnoringOtherApps,
        );
        if !ok {
            log::warn!("failed to re-activate source app pid {pid}");
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{
        AttachThreadInput, GetCurrentThreadId, GetWindowThreadProcessId,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

    pub fn capture() -> Option<isize> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                None
            } else {
                Some(hwnd.0 as isize)
            }
        }
    }

    pub fn activate(hwnd_raw: isize) {
        if hwnd_raw == 0 {
            return;
        }
        unsafe {
            let hwnd = HWND(hwnd_raw as *mut _);
            // SetForegroundWindow is restricted to the foreground process;
            // Quill IS the foreground process right now (the popup had
            // focus), but attaching the input threads makes the hand-off
            // reliable across Windows versions.
            let cur_thread = GetCurrentThreadId();
            let target_thread = GetWindowThreadProcessId(hwnd, None);
            let attached = if target_thread != 0 && target_thread != cur_thread {
                AttachThreadInput(cur_thread, target_thread, true).is_ok()
            } else {
                false
            };
            let _ = SetForegroundWindow(hwnd);
            if attached {
                let _ = AttachThreadInput(cur_thread, target_thread, false);
            }
        }
    }
}

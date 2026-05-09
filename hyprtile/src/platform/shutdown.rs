//! Helpers for shutting down Win32 message-pump threads.
//!
//! Keeps `WM_QUIT` posting and thread-id queries inside `platform/` so the
//! application coordinator does not need to import `windows-rs` types just to
//! signal worker threads to exit.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PostThreadMessageW, TranslateMessage, WM_QUIT,
};

/// Return the Win32 thread ID of the calling thread.
pub fn current_thread_id() -> u32 {
    unsafe { GetCurrentThreadId() }
}

/// Post `WM_QUIT` to the given Win32 thread to break its message loop.
///
/// Silently no-ops when `thread_id == 0` (i.e. the worker thread never
/// finished publishing its id) so callers do not need a guard.
pub fn post_quit_to_thread(thread_id: u32) {
    if thread_id == 0 {
        return;
    }
    unsafe {
        let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
    }
}

/// Run a Win32 thread-local `GetMessage` pump until `WM_QUIT` is received.
///
/// Used by background threads (event hook, hotkey listener) that need to keep
/// their Win32 callbacks alive. Returns when the loop terminates.
pub fn run_message_pump() {
    loop {
        unsafe {
            let mut msg = std::mem::zeroed();
            if GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                break;
            }
        }
    }
}

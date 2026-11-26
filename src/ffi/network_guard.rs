//! Installs the offline WebKit content rule list on macOS WebViews (native/network_guard.m).

use std::ffi::{c_char, c_void, CString};
use std::path::Path;
use wry::WebViewExtMacOS;

unsafe extern "C" {
    fn khana_install_offline_network_filter(controller: *mut c_void) -> i32;
    fn khana_macos_major_version() -> i32;
    fn khana_protect_data_dir(path: *const c_char);
    fn khana_set_window_sharing(allow: i32);
    fn khana_copy_transient(utf8: *const c_char, clear_secs: i32);
    fn khana_clear_transient();
}

/// Returns the real macOS major version via NSProcessInfo (immune to SYSTEM_VERSION_COMPAT).
pub fn macos_major_version() -> i32 {
    // SAFETY: Pure query on NSProcessInfo with no pointer arguments.
    unsafe { khana_macos_major_version() }
}

/// Excludes Khana's private data directory from Time Machine backups & Spotlight indexing (0700).
pub fn protect_data_dir(dir: &Path) {
    if let Ok(c) = CString::new(dir.to_string_lossy().as_bytes()) {
        // SAFETY: `c` is a valid NUL-terminated UTF-8 path string alive for the duration of the call.
        unsafe { khana_protect_data_dir(c.as_ptr()) };
    }
}

/// Controls macOS window screen-capture protection (`allow == false` -> `NSWindowSharingNone`).
pub fn set_window_sharing(allow: bool) {
    // SAFETY: Dispatches safely to the main queue with a plain integer flag.
    unsafe { khana_set_window_sharing(if allow { 1 } else { 0 }) };
}

/// Writes text to the macOS pasteboard as Transient + Concealed and auto-clears after `clear_secs`.
pub fn copy_transient(text: &str, clear_secs: i32) {
    if let Ok(c) = CString::new(text.as_bytes()) {
        // SAFETY: `c` is a valid NUL-terminated UTF-8 string copied inside `khana_copy_transient`.
        unsafe { khana_copy_transient(c.as_ptr(), clear_secs) };
    }
}

/// Immediately clears the macOS pasteboard if it still holds Khana's last copied text.
pub fn clear_transient() {
    // SAFETY: Dispatches safely to the main queue with no arguments.
    unsafe { khana_clear_transient() };
}

/// Install the default-deny offline network blocker before the WebView is allowed to load any page.
pub fn install(webview: &wry::WebView) {
    let controller = webview.manager();
    let controller_ptr = &*controller as *const _ as *mut c_void;
    // SAFETY: Wry returns the live WKUserContentController owned by this WebView; the helper
    // adds a retained WebKit rule list to that controller and does not keep the raw pointer.
    let installed = unsafe { khana_install_offline_network_filter(controller_ptr) };
    assert_eq!(installed, 1, "could not install the offline WebKit network filter");
}

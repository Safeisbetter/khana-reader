//! The Windows menu bar: giving the menu to the window is the one call in `muda` that needs `unsafe`.

/// Shows `menu` as the menu bar of the window whose handle is `hwnd`.
pub fn attach(menu: &muda::Menu, hwnd: isize) {
    // SAFETY: `hwnd` is the handle of the program's own top-level window (tao's), which is alive for as long as the program runs its
    // event loop, the only time the menu is used.
    unsafe {
        let _ = menu.init_for_hwnd(hwnd);
    }
}

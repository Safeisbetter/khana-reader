
#![forbid(unsafe_code)]
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
mod mac_main;
#[cfg(windows)]
mod win_main;

fn main() {
    #[cfg(not(windows))]
    mac_main::run();
    #[cfg(windows)]
    win_main::run();
}

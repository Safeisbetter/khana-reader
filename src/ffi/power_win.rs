//! Keeping the PC awake (not the screen) while a long job runs: the same as `SetThreadExecutionState` with
//! ES_SYSTEM_REQUIRED, which holds while the thread that asked lives and until it asks again.

use std::ffi::CStr;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetThreadExecutionState(flags: u32) -> u32;
}

const ES_CONTINUOUS: u32 = 0x8000_0000;
const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;

/// While this value lives, the PC does not go to sleep on its own.
pub struct Awake(());

impl Awake {
    pub fn new(_reason: &CStr) -> Option<Awake> {
        // SAFETY: SetThreadExecutionState only reads its flags.
        let before = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
        if before == 0 { None } else { Some(Awake(())) }
    }
}

impl Drop for Awake {
    fn drop(&mut self) {
        // SAFETY: as above: the thread's request is withdrawn.
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

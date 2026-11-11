
use std::path::Path;

pub fn reveal(p: &Path) {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSArray, NSString, NSURL};
    let s = NSString::from_str(&p.to_string_lossy());
    let url = NSURL::fileURLWithPath(&s);
    let ws = NSWorkspace::sharedWorkspace();
    ws.activateFileViewerSelectingURLs(&NSArray::from_retained_slice(&[url]));
}

pub fn heat_level() -> u8 {
    match objc2_foundation::NSProcessInfo::processInfo().thermalState().0 {
        2 => 2,
        n if n >= 3 => 3,
        _ => 0,
    }
}

pub fn battery() -> Option<(u8, bool)> {
    let out = std::process::Command::new("/usr/bin/pmset").args(["-g", "batt"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = text.lines().find(|l| l.contains("InternalBattery"))?;
    let pct: u8 = line.split('%').next()?.rsplit(|c: char| !c.is_ascii_digit()).next()?.parse().ok()?;
    let charging = text.contains("'AC Power'") || line.contains("; charging") || line.contains("; charged");
    Some((pct, charging))
}

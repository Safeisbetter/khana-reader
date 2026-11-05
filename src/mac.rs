
use std::path::Path;

pub fn reveal(p: &Path) {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSArray, NSString, NSURL};
    let s = NSString::from_str(&p.to_string_lossy());
    let url = NSURL::fileURLWithPath(&s);
    let ws = NSWorkspace::sharedWorkspace();
    ws.activateFileViewerSelectingURLs(&NSArray::from_retained_slice(&[url]));
}

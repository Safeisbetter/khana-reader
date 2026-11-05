
use crate::doc::Engines;

pub fn engines() -> Engines {
    Engines {
        ocr: Some(Box::new(|gray: &[u8], w: usize, h: usize| crate::ocr::ocr_gray(gray, w, h))),
        translate: if crate::translate::ready() {
            Some(Box::new(|paras: &[String], lang: &str| crate::translate::translate_page(paras, lang)))
        } else {
            None
        },
    }
}


use crate::pyre::*;

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    ReplaceWord { from: String, to: String },
    DeleteWord { from: String },
    DeleteParagraph,
}

fn plain(text: &str) -> String {
    let t = text.replace('ي', "ی").replace('ك', "ک").replace('\u{200c}', " ");
    split_ws(&t).join(" ")
}

const WORD: &str = r"(?:کلمه(?:\s+ی)?\s+)?";

pub fn parse_command(text: &str) -> Option<Command> {
    let t = plain(text);
    if search(r"^(?:این\s+)?بند(?:\s+را)?\s+(?:حذف|پاک)\s+کن$", &t) {
        return Some(Command::DeleteParagraph);
    }
    let pats = [
        format!(r"^{WORD}(.+?)\s+را\s+به\s+(.+?)\s+(?:تبدیل|تغییر|عوض)\s+(?:کن|بده)$"),
        format!(r"^{WORD}(.+?)\s+را\s+با\s+(.+?)\s+(?:عوض|جایگزین|عوض\s+کن|تبدیل)\s*(?:کن)?$"),
    ];
    for p in &pats {
        if let Some(g) = fullmatch_groups(p, &t) {
            let from = g.get(1).and_then(|x| x.clone())?.trim().to_string();
            let to = g.get(2).and_then(|x| x.clone())?.trim().to_string();
            if !from.is_empty() && !to.is_empty() {
                return Some(Command::ReplaceWord { from, to });
            }
        }
    }
    if let Some(g) = fullmatch_groups(&format!(r"^{WORD}(.+?)\s+را\s+(?:حذف|پاک)\s+کن$"), &t) {
        let from = g.get(1).and_then(|x| x.clone())?.trim().to_string();
        if !from.is_empty() && from != "این بند" {
            return Some(Command::DeleteWord { from });
        }
    }
    None
}

pub fn command_word(text: &str) -> Option<&'static str> {
    let t = plain(text);
    if search(r"بند", &t) && search(r"(?:عوض|تغییر|تبدیل|بنویس|بازنویسی|جایگزین)", &t) {
        Some("rewrite")
    } else if search(r"اضافه", &t) {
        Some("ezafe")
    } else if search(r"جدا", &t) {
        Some("split")
    } else if search(r"(?:تلفظ|جور|جوری|بخوان|خوانده)", &t) {
        Some("say")
    } else if search(r"بند", &t) && search(r"(?:حذف|پاک)", &t) {
        Some("paragraph")
    } else if search(r"(?:حذف|پاک)", &t) {
        Some("delete")
    } else if search(r"(?:عوض|تبدیل|تغییر)", &t) {
        Some("replace")
    } else {
        None
    }
}

pub fn is_yes(text: &str) -> bool {
    let t = plain(text);
    !search(r"(?:نه|خیر|نادرست|لغو|غلط)", &t) && search(r"(?:^|\s)(?:بله|آره|اره|بلی|درست|تایید|تأیید|باشه|انجام\s+بده|انجام\s+بدهید)(?:\s|$)", &t)
}


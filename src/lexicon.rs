
use crate::consts::*;
use crate::difflib::{Op, SequenceMatcher};
use crate::g2p::OnnxG2P;
use crate::phon::{pronounce, pronounce_word, PHONEME_WORD};
use crate::pyre::*;
use crate::text::repair_default;
use crate::vocab::{latin_phonemes, source_is_dutch};
use std::collections::HashMap;
use std::sync::Mutex;

type R = Result<String, String>;

/// True when every letter of a Persian word has its own vowel mark (or sukun), except the letters that can be long vowels
/// (ا آ و ی ه), which may stand bare. Such a word can be said from its marks alone.
pub fn fully_marked(word: &str) -> bool {
    let mut letters: Vec<(char, bool)> = Vec::new();
    for c in word.chars() {
        if ('\u{064b}'..='\u{0652}').contains(&c) {
            if let Some(l) = letters.last_mut() {
                if matches!(c, '\u{064e}' | '\u{0650}' | '\u{064f}' | '\u{0652}') {
                    l.1 = true;
                }
            }
        } else if ('\u{0600}'..='\u{06ff}').contains(&c) {
            letters.push((c, false));
        }
    }
    !letters.is_empty() && letters.iter().all(|(c, marked)| *marked || "اآوییه".contains(*c))
}

pub fn collapse_loops(ph: &str) -> String {
    let mut words: Vec<String> = split_ws(ph).iter().map(|s| s.to_string()).collect();
    for n in 1..5 {
        let mut i = 0;
        let mut out: Vec<String> = Vec::new();
        while i < words.len() {
            let unit: Vec<String> = words[i..(i + n).min(words.len())].to_vec();
            let mut k = 1;
            while unit.len() == n && words.get(i + k * n..(i + (k + 1) * n).min(words.len())).map_or(false, |s| s == unit.as_slice()) {
                k += 1;
            }
            if k >= 3 && unit.len() == n {
                out.extend(unit);
                i += k * n;
            } else {
                out.push(words[i].clone());
                i += 1;
            }
        }
        words = out;
    }
    words.join(" ")
}

fn skeleton(ph: &str) -> String {
    sub(r"[aeoiuAy?1;\-]", "", &ph.to_lowercase())
}

pub fn core(word: &str) -> String {
    sub(r"^[^\w]+|[^\w‌]+$", "", word).replace('\u{064b}', "")
}

pub fn consonants_ok(word: &str, ph: &str) -> bool {
    let mut want = String::new();
    for c in word.chars() {
        let m = match c {
            'ب' => 'b', 'پ' => 'p', 'ت' | 'ط' => 't', 'ث' | 'س' | 'ص' => 's', 'ج' => 'j', 'چ' => 'C', 'خ' => 'x', 'د' => 'd',
            'ذ' | 'ز' | 'ض' | 'ظ' => 'z', 'ر' => 'r', 'ژ' => ';', 'ش' => 'S', 'غ' | 'ق' => 'q', 'ف' => 'f', 'ک' | 'ك' => 'k',
            'گ' => 'g', 'ل' => 'l', 'م' => 'm', 'ن' | '\u{064B}' => 'n',
            'ا' | 'آ' | 'و' | 'ی' | 'ي' | 'ى' | 'ه' | 'ة' | 'ۀ' | 'ع' | 'ء' | 'أ' | 'إ' | 'ئ' | 'ؤ' | 'ح' | '\u{200c}' => continue,
            c if ('\u{064C}'..='\u{0652}').contains(&c) || c == '\u{0670}' => continue,
            _ => return true,
        };
        if !want.ends_with(m) { want.push(m); }
    }
    let ends_yeh = word.ends_with(['ی', 'ي']) && word.chars().rev().nth(1).is_some_and(|c| !"اآوی‌".contains(c) && c != '\u{200c}');
    if ends_yeh && word.chars().count() >= 3 {
        let r = ph.trim_end_matches(['1', '-']);
        let r = r.strip_suffix("ye").unwrap_or(r);
        if !r.ends_with(['i', 'y', 'A']) { return false; }
    }
    if word.ends_with('ا') && !want.is_empty() && ph.trim_end_matches('1').ends_with("an") && !want.ends_with('n') { want.push('n'); }
    let ph = ph.replace("gh", "q");
    let mut got = String::new();
    for c in ph.chars().filter(|c| "bptsjCxdzr;Sqfkglmn".contains(*c)) {
        if !got.ends_with(c) { got.push(c); }
    }
    want == got
}

pub fn no_ezafe_before_verb(ph: &str) -> String {
    const VERBS: &[&str] = &["kard", "kardam", "kardi", "kardim", "kardid", "kardand", "Sod", "Sodam", "Sodi", "Sodim", "Sodid", "Sodand",
        "bud", "budam", "budi", "budim", "budid", "budand", "raft", "raftam", "raftand", "dAd", "dAdam", "dAdand", "goft", "goftam",
        "goftand", "dASt", "dAStam", "dAStand", "?Amad", "?Amadam", "?Amadand", "mord", "mordand", "gereft", "gereftam", "gereftand",
        "?ast", "hast", "nist", "hastand", "nistand", "hastam", "nistam"];
    const NUMBERS: &[&str] = &["yek", "do", "se", "CahAr", "panj", "SeS", "haft", "haSt", "noh", "dah", "yAzdah", "davAzdah", "sizdah",
        "CahArdah", "pAnzdah", "SAnzdah", "hefdah", "hejdah", "nuzdah", "bist", "si", "Cehel", "panjAh", "haftAd", "haStAd", "navad"];
    let mut w: Vec<String> = split_ws(ph).iter().map(|s| s.to_string()).collect();
    for i in 0..w.len().saturating_sub(1) {
        let number = NUMBERS.contains(&w[i].trim_end_matches("e1")) && !NUMBERS.contains(&w[i + 1].as_str()) && w[i + 1] != "?o";
        if (VERBS.contains(&w[i + 1].as_str()) || number) && w[i].ends_with("e1") && (w[i].chars().count() > 3 || number) {
            let base = &w[i][..w[i].len() - 2];
            let base = if base.ends_with('y') && base[..base.len() - 1].ends_with(['A', 'u', 'o', 'i', 'e']) { &base[..base.len() - 1] } else { base };
            w[i] = base.to_string();
        }
    }
    w.join(" ")
}

pub fn align_words(words: &[String], phw: &[String]) -> Vec<(usize, usize)> {
    if words.len() == phw.len() {
        return (0..words.len()).map(|i| (i, i)).collect();
    }
    let a: Vec<String> = words.iter().map(|w| skeleton(&pronounce_word(&sub(r"[^\w‌]", "", w)))).collect();
    let b: Vec<String> = phw.iter().map(|p| skeleton(p)).collect();
    let mut out: Vec<(usize, usize)> = Vec::new();
    let set = |i: usize, j: usize, out: &mut Vec<(usize, usize)>| match out.iter_mut().find(|x| x.0 == i) {
        Some(x) => x.1 = j,
        None => out.push((i, j)),
    };
    for (op, i1, i2, j1, j2) in SequenceMatcher::new(&a, &b, false).get_opcodes() {
        match op {
            Op::Equal => {
                for k in 0..i2 - i1 {
                    set(i1 + k, j1 + k, &mut out);
                }
            }
            Op::Replace => {
                for k in 0..(i2 - i1).min(j2 - j1) {
                    if crate::difflib::ratio_str(&a[i1 + k], &b[j1 + k], true) >= 0.7 {
                        set(i1 + k, j1 + k, &mut out);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn lookup<'a>(m: &'a [(&str, &str)], k: &str) -> Option<&'a str> {
    m.iter().find(|(a, _)| *a == k).map(|(_, v)| *v)
}

const GENERAL_LEXICON: &[(&str, &str)] = &[
    ("نماگرفت", "nemAgereft"),
    ("نمایه", "nemAye"),
    ("نماهنگ", "nemAhang"),
    ("نماپاد", "nemApAd"),
    ("فرانما", "farAnemA"),
    ("گپ", "gap"),
    ("رایانک", "rAyAnak"),
    ("تارنگاشت", "tArnegASt"),
    ("تصدانه", "tasdAne"),
    ("چندرسانهای", "Candresane?i"),
    ("چندرسانه‌ای", "Candresane?i"),
    ("پیام چندرسانه‌ای", "payAme Candresane?i"),
    ("پیام چندرسانهای", "payAme Candresane?i"),
    ("نمایه‌ها", "nemAyehA"),
    ("نمایهها", "nemAyehA"),
    ("نماگرفت‌ها", "nemAgerefthA"),
    ("نماگرفتها", "nemAgerefthA"),
    ("گوگل", "gugel"),
    ("اینستاگرام", "?instAgerAm"),
    ("کامنت", "kAment"),
    ("فالوور", "fAlover"),
    ("هشتگ", "haStag"),
    ("اندروید", "?andoroyd"),
    ("اکانت", "?akAnt"),
    ("پسورد", "pasvord"),
    ("یوزرنیم", "yuzernim"),
    ("هکر", "haker"),
    ("وی پی ان", "vipiyen"),
    ("ویپیان", "vipiyen"),
    ("وی‌پی‌ان", "vipiyen"),
    ("بلوتوث", "bolutus"),
    ("بارکد", "bArkod"),
    ("کیوآرکد", "kiyuArkod"),
    ("اکسل", "?eksel"),
    ("نوتیفیکیشن", "notifikeySen"),
    ("تارنگار", "tArnegAr"),
    ("بارگذاری", "bArgozAri"),
    ("صفحهکلید", "safheklid"),
    ("صفحه‌کلید", "safheklid"),
    ("صفحه کلید", "safheklid"),
    ("رایانش", "rAyAneS"),
    ("رایانش ابری", "rAyAneSe ?abri"),
    ("بروم", "beravam"),
    ("کشتی", "keSti"), ("کشتی‌ها", "keStihA"), ("کشتی گرفتن", "koSti gereftan"), ("کشتی گرفت", "koSti gereft"),
    ("کشتی گرفتند", "koSti gereftand"), ("کشتی می‌گیرند", "koSti migirand"), ("کشتی می‌گرفتند", "koSti migereftand"),
    ("کشتی آزاد", "koSti ?AzAd"), ("کشتی فرنگی", "koSti farangi"),
    ("نکند", "nakonad"), ("نکنی", "nakoni"), ("نکنم", "nakonam"), ("نکنند", "nakonand"), ("بکنید", "bekonid"), ("بکنم", "bekonam"),
    ("بکنی", "bekoni"), ("بکند", "bekonad"), ("بکنیم", "bekonim"), ("بکنند", "bekonand"),
    ("چهارشنبه‌سوری", "CahArSanbesuri"), ("چهارشنبه سوری", "CahArSanbesuri"),
    ("شوید", "Savid"), ("نشوید", "naSavid"), ("جلد", "jeld"), ("کبد", "kabed"), ("مایل", "mAyel"), ("پنل", "panel"), ("پانل", "pAnel"),
    ("فقه", "feqh"), ("جور", "jur"), ("جزو", "jozv"), ("مجتمع", "mojtama?"), ("السلام", "?assalAm"), ("بده", "bede"), ("نیا", "nayA"),
    ("شرب", "Sorb"), ("نوسان", "navasAn"), ("مشرق", "maSreq"), ("غدد", "qodad"), ("وزرا", "vozarA"), ("وزرای", "vozarAye"),
    ("روسای", "ro?asAye"), ("رؤسای", "ro?asAye"), ("روسا", "ro?asA"), ("رؤسا", "ro?asA"), ("مصدق", "mosaddeq"), ("مهدوی", "mahdavi"),
    ("روبه‌رو", "ruberu"), ("روبرو", "ruberu"), ("روبه رو", "ruberu"), ("جست‌وجو", "jostoju"), ("جستجو", "jostoju"), ("جست و جو", "jostoju"),
    ("گفت‌وگو", "goftogu"), ("گفتگو", "goftogu"), ("گفت و گو", "goftogu"), ("ارایه", "?erA?e"), ("ارائه", "?erA?e"), ("هیات", "hey?at"),
    ("هیأت", "hey?at"), ("هیئت", "hey?at"), ("گاها", "gAhan"), ("گاهاً", "gAhan"), ("ناچارا", "nACAran"), ("ناچاراً", "nACAran"),
    ("ایرانسل", "?irAnsel"),
    ("ارگ", "?arg"), ("مشترکان", "moStarekAn"), ("سرب", "sorb"), ("لکن", "lAken"), ("گریم", "gerim"), ("راهور", "rAhvar"), ("فورا", "foran"),
    ("فوراً", "foran"), ("یائسگی", "yA?esegi"), ("متد", "metod"), ("هفدهم", "hefdahom"), ("مجرای", "majrAye"), ("شنود", "Senud"),
    ("نبوت", "nobovvat"), ("نصاب", "nesAb"), ("شعاع", "So?A?"), ("مبدأ", "mabda?"), ("مبدا", "mabda?"), ("پرشین", "parSin"),
    ("مولف", "mo?allef"), ("مؤلف", "mo?allef"), ("بسم", "besm"), ("هیدروژن", "hidro;en"), ("مستر", "mester"), ("یوونتوس", "yuventus"),
    ("بطن", "batn"), ("صحیحی", "sahihi"), ("اهلبیت", "?ahlebeyt"), ("اهل‌بیت", "?ahlebeyt"), ("سیدالشهدا", "seyyedoSSohadA"),
    ("ارس", "?aras"), ("فول", "ful"), ("محول", "mohavval"), ("حکام", "hokkAm"),
    ("اسپم", "?espam"), ("بیع", "bey?"), ("ضایعه", "zAye?e"), ("گود", "gowd"), ("خلیج‌فارس", "xalije fArs"), ("خلیجفارس", "xalije fArs"),
    ("خلیج فارس", "xalije fArs"), ("سهیل", "sohayl"), ("طیبه", "tayyebe"), ("نامجو", "nAmju"), ("معراج", "me?rAj"),
    ("عبدالعزیز", "?abdol?aziz"), ("کوانتومی", "kuAntomi"), ("زیپ", "zip"), ("مسطح", "mosattah"), ("شمش", "SemS"), ("پُر", "por"),
    ("احد", "?ahad"), ("تاسوعا", "tAsu?A"), ("السّلام", "?assalAm"), ("سپه", "sepah"), ("گجت", "gajet"), ("چشمم", "CeSmam"),
    ("موید", "mo?ayyed"), ("مؤید", "mo?ayyed"), ("سه‌گانه", "segAne"), ("سهگانه", "segAne"), ("رؤیت", "ro?yat"), ("رویت", "ro?yat"),
    ("ولوو", "volvo"), ("راکد", "rAked"), ("متعاقبا", "mote?Aqeban"), ("متعاقباً", "mote?Aqeban"), ("معاند", "mo?Aned"),
    ("کرملین", "kremlin"),
    ("بایت", "bAyt"), ("کمیل", "komeyl"), ("بهسوی", "besuye"), ("به‌سوی", "besuye"), ("مبرم", "mobram"), ("هلال‌احمر", "helAle ahmar"),
    ("هلالاحمر", "helAle ahmar"), ("هلال احمر", "helAle ahmar"), ("هفدهمین", "hefdahomin"), ("ستار", "sattAr"), ("ستارخان", "sattArxAn"),
    ("لود", "lod"), ("ماکرون", "mAkron"), ("ملقب", "molaqqab"), ("تأویل", "ta?vil"), ("اوستا", "?avestA"), ("مصلای", "mosallAye"),
    ("مصلی", "mosallA"), ("مرودشت", "marvdaSt"), ("شهرری", "Sahre rey"), ("شهر ری", "Sahre rey"), ("والامقام", "vAlAmaqAm"),
    ("دیون", "doyun"), ("هامبورگ", "hAmburg"), ("زیمنس", "zimens"), ("وبر", "veber"), ("لیبل", "leybel"), ("تیمم", "tayammom"),
    ("ازن", "?ozon"), ("بهموقع", "bemowqe?"), ("به‌موقع", "bemowqe?"), ("عبارتاند", "?ebAratand"), ("عبارت‌اند", "?ebAratand"),
    ("مردد", "moradded"), ("سهیلی", "sohayli"), ("سهیلا", "soheylA"), ("امثالهم", "?amsAlehem"), ("نهجالبلاغه", "nahjolbalAqe"),
    ("نهج‌البلاغه", "nahjolbalAqe"), ("اینشتین", "?inSteyn"),
    ("مبداء", "mabda?"), ("میلر", "miler"), ("مشاع", "moSA?"), ("تیمملی", "time melli"), ("پلیر", "pleyer"), ("مفتح", "mofatteh"),
    ("مفسد", "mofsed"), ("لوپز", "lopez"), ("کوانتوم", "kuAntom"), ("بیوقفه", "biveqfe"), ("بی‌وقفه", "biveqfe"), ("مجد", "majd"),
    ("المنت", "?elemAnt"), ("برگزیت", "bregzit"), ("هاوکینگ", "hAking"), ("میایستد", "miistad"), ("می‌ایستد", "miistad"),
    ("کارن", "kAren"), ("جوید", "juyid"), ("کمنظیر", "kamnazir"), ("کم‌نظیر", "kamnazir"), ("فوتون", "foton"), ("مشرک", "moSrek"),
    ("پرشیا", "parSiyA"),
    ("اخص", "?axass"), ("درایران", "dar ?irAn"), ("بهسمت", "besamte"), ("به‌سمت", "besamte"), ("دبلیو", "dabelyu"), ("پتنت", "patent"),
    ("دایرکت", "dAyrekt"), ("نساء", "nesA?"), ("سیدالشهداء", "seyyedoSSohadA"), ("کلود", "klod"), ("فولادوند", "fulAdvand"),
    ("میشکند", "miSkanad"), ("می‌شکند", "miSkanad"), ("شیوخ", "Soyux"), ("منصه", "manasse"), ("امل", "?amal"), ("هاتف", "hAtef"),
    ("تأسی", "ta?assi"), ("دجال", "dajjAl"), ("فرقان", "forqAn"), ("حبیبالله", "habibollAh"), ("حبیب‌الله", "habibollAh"),
    ("ملانین", "melAnin"), ("قصهگویی", "qesseguyi"), ("قصه‌گویی", "qesseguyi"), ("ندبه", "nodbe"), ("مجرا", "majrA"),
    ("محرمات", "moharramAt"), ("مصراع", "mesrA?"), ("گلر", "goler"), ("عبدالعلی", "?abdol?ali"), ("شمر", "Semr"),
    ("متدولوژی", "metodolo;i"), ("تراورتن", "trAverten"), ("پرنسس", "prenses"), ("عالیرتبه", "?Alirotbe"), ("عالی‌رتبه", "?Alirotbe"), ("افکت", "?efekt"), ("گیتار", "gitAr"), ("عفو", "?afv"), ("اینست", "?inast"), ("والمسلمین", "valmoslemin"),
    ("بروی", "beravi"),
    ("یک و نیم", "yeko nim"),
    ("دو و نیم", "do?o nim"),
    ("سه و نیم", "se?o nim"),
    ("چهار و نیم", "cAhAro nim"),
    ("پنج و نیم", "panjo nim"),
    ("شش و نیم", "SiSo nim"),
    ("هفت و نیم", "hafto nim"),
    ("هشت و نیم", "haSto nim"),
    ("نه و نیم", "noho nim"),
    ("ده و نیم", "daho nim"),
    ("یازده و نیم", "yAzdaho nim"),
    ("دوازده و نیم", "davAzdaho nim"),
    ("کلاه بردار", "kolAhbardAr"),
    ("پی آمد", "piyAmad"),
    ("پی آمدها", "piyAmadhA"),
    ("پی آمدهای", "piyAmadhAye"),
    ("آن لاین", "?Anlayn"),
    ("پر قو", "pare qu"),
    ("ضرب مثل", "zarbe masal"),
    ("کش و قوس", "keS ?o qos"),
    ("جهان بینی", "jahAnbini"),
    ("شی", "Sey"),
    ("از کل", "?az koll"),
    ("بخشی از کل", "baxSi ?az koll"),
    ("اشکال جدید", "?aSkAle jadid"),
    ("اشکال جدیدی", "?aSkAle jadidi"),
    ("اشکال خارجی", "?aSkAle xAreji"),
    ("اشکال مختلف", "?aSkAle moxtalef"),
    ("اشکال دیگر", "?aSkAle digar"),
    ("سپتامبر", "septAmbr"),
    ("اکتبر", "?oktobr"),
    ("نوامبر", "novAmbr"),
    ("دسامبر", "desAmbr"),
    ("تحت تاثیر", "tahte ta?sire"),
    ("تحت تأثیر", "tahte ta?sire"),
    ("هفته نامه", "haftenAme"),
    ("ویژه نامه", "vi;enAme"),
    ("گام جلوتر", "gAm jelotar"),
    ("دار و دسته", "dAr o daste"),
    ("هرج و مرج", "harj o marj"),
    ("با فرض اینکه", "bA farze ?inke"),
    ("فرض اینکه", "farze ?inke"),
    ("گو اینکه", "gu ?inke"),
    ("مبتنی بر", "mabtani bar"),
    ("چاره را", "CAre rA"),
    ("مورد ترحّم", "morede tarahhom"),
    ("مورد ترحم", "morede tarahhom"),
    ("مرد یا زن", "mard yA zan"),
    ("زن یا مرد", "zan yA mard"),
    ("اتنیک", "?etnik"),
    ("اتنیکی", "?etniki"),
    ("مترسکی", "matarsaki"),
    ("مترسک‌ها", "matarsakhA"),
    ("مترسکها", "matarsakhA"),
    ("رأی", "ra?y"),
    ("رأی‌ها", "ra?yhA"),
    ("رأیها", "ra?yhA"),
    ("مسئولان", "mas?ulAn"),
    ("مسئولین", "mas?ulin"),
    ("مسوولان", "mas?ulAn"),
    ("مسوولین", "mas?ulin"),
    ("علیکم", "?aleykom"),
    ("السلام علیکم", "?assalAmo ?aleykom"),
    ("سلام علیکم", "salAmo ?aleykom"),
    ("علیکم السلام", "?aleykomossalAm"),
    ("رایانهای", "rAyAne?i"),
    ("رایانه‌ای", "rAyAne?i"),
    ("رایانه ای", "rAyAne?i"),
    ("خودنگاره", "xodnegAre"),
    ("دورنگار", "durnegAr"),
    ("الگوریتم", "?algoritm"),
    ("استوری", "?estori"),
    ("دانلود", "dAnlod"),
    ("آپلود", "?Aplod"),
    ("اینستاگرامی", "?instAgerAmi"),
    ("دانلودها", "dAnlodhA"),
    ("الگوریتم ها", "?algoritmhA"),
    ("الگوریتم‌ها", "?algoritmhA"),
    ("الگوریتمها", "?algoritmhA"),
    ("قویا", "qaviyyan"),
    ("کلاهبردار", "kolAhbardAr"),
    ("آنلاین", "?Anlayn"),
    ("گنگ", "gong"),
    ("شق", "Seqq"),
    ("سنخ", "senx"),
    ("ترکه", "tarke"),
    ("مترسک", "matarsak"),
    ("کلنگ", "kolang"),
    ("ولگرد", "velgard"),
    ("سنگفرش", "sangfarS"),
    ("سوءظن", "su?ezan"),
    ("چکش", "CakoS"),
    ("انبر", "?anbor"),
    ("عالمه", "?Aleme"),
    ("وازلین", "vAzelin"),
    ("مثل", "mesle"),
    ("باربر", "bArbar"),
    ("شش", "SeS"),
    ("پرتره", "portre"),
    ("شیء", "Sey?"),
    ("منافع", "manAfe?e"),
    ("درباره", "darbAreye"),
    ("نکنند", "nakonand"),
    ("رای", "ra?y"),
    ("کریکت", "kriket"),
    ("قایقرانی", "qAyeqrAni"),
    ("رویارویی", "ruyAruyi"),
    ("مونتاژ", "montA;"),
    ("پروژکتور", "porojektor"),
    ("تألیفی", "ta?lifi"),
    ("تالیفی", "ta?lifi"),
    ("چاره", "CAre"),
    ("انهدام", "?enhedAm"),
    ("وانگهی", "vAngahi"),
    ("درکی", "darki"),
    ("بدوی", "badavi"),
    ("مضحک", "mozahek"),
    ("حول", "havl"),
    ("کردار", "kardAr"),
    ("موطن", "mavtan"),
    ("بنگریم", "bengarim"),
    ("ممارست", "momArast"),
    ("جبلی", "jebelli"),
    ("ادعیه", "?ad?iye"),
    ("اینده", "?Ayande"),
    ("گوتیک", "gotik"),
    ("پژوهش‌گران", "pa;uheSgarAn"),
    ("ماکیان", "mAkiyAn"),
    ("علیرغم", "?alA raqme"),
    ("سحر", "sahar"),
    ("قرمه‌سبزی", "qormesabzi"),
    ("استریت", "?estreyt"),
];

const RUST_LEXICON: &[(&str, &str)] = &[
];

const EZAFE_PRONE: &[&str] = &["ستم", "نوع", "ضلع", "فرع"];
const EZAFE_STOP: &[&str] = &[
    "و", "را", "که", "از", "در", "به", "با", "تا", "یا", "است", "نیست", "بود", "شد", "شده", "می", "نمی", "هم", "نیز", "بر", "بی", "چون", "اما", "ولی", "اگر", "آنکه", "هیچ", "نه", "هر", "چه", "پس", "بلکه",
];

fn ezafe_after_ain(w: &str) -> String {
    sub(r"^(.+?[uoAi])(?:ye1?|e)\?e(1?)$", r"\1?e\2", w)
}


fn mikon_fixed(w: &str) -> String {
    sub(r"^(ne)?mikan(am|i|ad|im|id|and)$", r"\1mikon\2", w)
}


fn has_vowel(p: &str) -> bool {
    p.chars().any(|c| "aeoiuA".contains(c))
}

fn ends_any(s: &str, e: &[&str]) -> bool {
    e.iter().any(|x| s.ends_with(x))
}

pub struct LexiconG2P {
    pub base: OnnxG2P,
    lexicon: Mutex<(Vec<(String, String)>, Option<&'static fancy_regex::Regex>)>,
    plain: Mutex<HashMap<String, String>>,
    memo: Mutex<HashMap<(String, bool), String>>,
}

impl LexiconG2P {
    pub fn new(base: OnnxG2P) -> Self {
        LexiconG2P { base, lexicon: Mutex::new((Vec::new(), None)), plain: Mutex::new(HashMap::new()), memo: Mutex::new(HashMap::new()) }
    }

    fn spelling_phonemes(&self, spelling: &str) -> R {
        let spelling = strip(spelling);
        if spelling.is_empty() {
            return Ok(String::new());
        }
        if search(PHONEME_WORD, &spelling.replace(' ', "")) || search(r"[ً-ْ]", spelling) {
            return Ok(pronounce(spelling));
        }
        if let Some(p) = self.plain.lock().unwrap().get(spelling) {
            return Ok(p.clone());
        }
        let p = self.phonemise_marked(&repair_default(spelling), true)?;
        self.plain.lock().unwrap().insert(spelling.to_string(), p.clone());
        Ok(p)
    }

    pub fn set_lexicon(&self, user: &[(String, String)]) {
        let mut all: Vec<(String, String)> =
            BUILTIN_LEXICON.iter().chain(GENERAL_LEXICON).chain(RUST_LEXICON).map(|(a, b)| (a.to_string(), b.to_string())).collect();
        for (w, s) in user {
            match all.iter_mut().find(|x| &x.0 == w) {
                Some(x) => x.1 = s.clone(),
                None => all.push((w.clone(), s.clone())),
            }
        }
        let mut lex: Vec<(String, String)> = Vec::new();
        for (word, spelling) in all {
            let word = repair_default(&word);
            let ph = self.spelling_phonemes(&spelling).unwrap_or_default();
            if !word.is_empty() && !ph.is_empty() {
                match lex.iter_mut().find(|x| x.0 == word) {
                    Some(x) => x.1 = ph,
                    None => lex.push((word, ph)),
                }
            }
        }
        let mut cur = self.lexicon.lock().unwrap();
        if lex != cur.0 {
            let mut words: Vec<&String> = lex.iter().map(|x| &x.0).collect();
            words.sort_by(|a, b| len(b).cmp(&len(a)));
            let re = if words.is_empty() {
                None
            } else {
                let alt = words.iter().map(|w| fancy_regex::escape(w).into_owned()).collect::<Vec<_>>().join("|");
                Some(rx(&format!("(?<![\\w\u{200c}])({alt})(?![\\w\u{200c}])")))
            };
            *cur = (lex, re);
            self.memo.lock().unwrap().clear();
        }
    }

    pub fn phonemise(&self, text: &str, keep_ezafe: bool) -> R {
        let key = (if source_is_dutch() { format!("nl|{text}") } else { text.to_string() }, keep_ezafe);
        if let Some(p) = self.memo.lock().unwrap().get(&key) {
            return Ok(p.clone());
        }
        let mut r = self.phonemise_uncached(text, keep_ezafe)?;
        // A stray leading «ye» (from a linking «ی» that was split off the word before) is not a sound a word starts with,
        // unless the text itself starts with ی: دریای سیاه was read «ye daryAye siyAh».
        if !text.trim_start().starts_with(['ی', 'ي']) {
            if let Some(rest) = r.strip_prefix("ye ").or_else(|| r.strip_prefix("ye1 ")) {
                r = rest.to_string();
            }
        }
        let mut m = self.memo.lock().unwrap();
        if m.len() > 20000 {
            m.clear();
        }
        m.insert(key, r.clone());
        Ok(r)
    }

    fn phonemise_uncached(&self, text: &str, keep_ezafe: bool) -> R {
        let mut text = sub(r"ق\.\s?م\.?", "قبل از میلاد", text);
        text = sub(r"([\u0600-\u06FF]{2,})ه[ \u200c](گی|گان|گانی)(?![\u0600-\u06FF])", "\\1\\2", &text);
        text = sub(r"(?<=[\u0600-\u06FF]ه) +(ی|ای)(?![\u0600-\u06FF\u200c])", "\u{200c}\\1", &text);
        text = sub(r"(?<=[\d۰-۹٠-٩]) قم(?![\w])", " قبل از میلاد", &text);
        text = sub(r"(?<=[\d۰-۹٠-٩])\s*ص\.?(?![\w])", " صفحه", &text);
        text = sub(r"(?<![\d۰-۹٠-٩])([\d۰-۹٠-٩]{2,4})\s*[-–]\s*([\d۰-۹٠-٩]{2,4})(?![\d۰-۹٠-٩])", r"\1 تا \2", &text);
        let mut found: Vec<(usize, usize, String)> = Vec::new();
        {
            let lex = self.lexicon.lock().unwrap();
            if let Some(re) = lex.1 {
                for c in re.captures_iter(text.as_str()).flatten() {
                    let m = c.get(0).unwrap();
                    let ph = lex.0.iter().find(|x| x.0 == &c[1]).map(|x| x.1.clone()).unwrap_or_default();
                    found.push((m.start(), m.end(), ph));
                }
            }
        }
        for m in rx(r"(?<![\d۰-۹٠-٩٫٬.,/])[9۹٩](?![\d۰-۹٠-٩٫٬]|[.,/][\d۰-۹٠-٩])").find_iter(text.as_str()).flatten() {
            if !found.iter().any(|(a, b, _)| *a < m.end() && m.start() < *b) {
                found.push((m.start(), m.end(), "noh".into()));
            }
        }
        // A Persian word that has its own vowel marks (fatha, kasra or damma, as an author or a fix writes them) is said as
        // those marks say. The general reader ignored the marks: مِهر was read «mohr».
        for m in rx(r"(?<![؀-ۿ‌])[؀-ۿ‌ً-ْ]*[َُِ][؀-ۿ‌ً-ْ]*").find_iter(text.as_str()).flatten() {
            // (only a word whose sounds are all marked: with a letter left unmarked, the marks alone would drop its vowel)
            if fully_marked(m.as_str()) && !found.iter().any(|(a, b, _)| *a < m.end() && m.start() < *b) {
                let ph = pronounce(m.as_str());
                if !ph.is_empty() {
                    found.push((m.start(), m.end(), ph));
                }
            }
        }
        for m in rx(LATIN_WORD).find_iter(text.as_str()).flatten() {
            if let Some(ph) = latin_phonemes(m.as_str()) {
                if !found.iter().any(|(a, b, _)| *a < m.end() && m.start() < *b) {
                    found.push((m.start(), m.end(), ph));
                }
            }
        }
        if found.is_empty() {
            return self.phonemise_marked(&text, keep_ezafe);
        }
        found.sort();
        let mut parts: Vec<String> = Vec::new();
        let mut pos = 0;
        for (start, end, ph) in found {
            let seg = &text[pos..start];
            if search(r"^[\s،,]*و[\s،,]*$", seg) {
                parts.push("va".into());
            } else if search(r"\w", seg) {
                let span = &text[start..end];
                let mut nxt: Vec<String> = split_ws(&self.phonemise_marked(span, keep_ezafe)?).iter().map(|s| s.to_string()).collect();
                let mut cut: Vec<String> = split_ws(&self.phonemise_marked(&format!("{} {span}", rstrip(seg)), keep_ezafe)?)
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                let n_seg = if search(r"[\d۰-۹٠-٩A-Za-z]", seg) { 1 } else { split_ws(seg).iter().filter(|w| search(r"\w", w)).count() };
                if nxt.is_empty() || cut.len() < nxt.len() + n_seg {
                    cut = split_ws(&self.phonemise_marked(&format!("{} او", rstrip(seg)), keep_ezafe)?).iter().map(|s| s.to_string()).collect();
                    nxt = cut.last().cloned().into_iter().collect();
                }
                if span.contains('\u{200c}') && cut.len() > nxt.len() + n_seg {
                    cut = split_ws(&self.phonemise_marked(&format!("{} او", rstrip(seg)), keep_ezafe)?).iter().map(|s| s.to_string()).collect();
                    nxt = cut.last().cloned().into_iter().collect();
                }
                let k = nxt.len();
                if cut.len() >= k + n_seg {
                    parts.push(cut[..cut.len() - k].join(" "));
                } else {
                    parts.push(self.phonemise_marked(seg, keep_ezafe)?);
                }
            }
            parts.push(self.listed_ezafe(&text, start, end, ph, keep_ezafe)?);
            pos = end;
        }
        let tail = &text[pos..];
        if search(r"\w", tail) {
            let lone = split_ws(tail).iter().filter(|w| search(r"\w", w)).count() == 1;
            let after: Vec<String> = if lone {
                split_ws(&self.phonemise_marked(&format!("او {}", tail.trim_start()), keep_ezafe)?).iter().skip(1).map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            };
            if after.len() == 1 {
                parts.push(after[0].clone());
            } else {
                parts.push(self.phonemise_marked(tail, keep_ezafe)?);
            }
        }
        Ok(parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" "))
    }

    fn listed_ezafe(&self, text: &str, start: usize, end: usize, ph: String, keep_ezafe: bool) -> R {
        let span = &text[start..end];
        let Some(last) = split_ws(&ph).last().map(|s| s.to_string()) else { return Ok(ph) };
        if !search(&format!("[{FA_LETTER}]"), span) || last.ends_with('1') {
            return Ok(ph);
        }
        let rest = &text[end..];
        if !search(&format!(r"^[ \t]+[{FA_LETTER}]"), rest) {
            return Ok(ph);
        }
        let next: Vec<&str> = split_ws(rest).into_iter().take(2).collect();
        if next.is_empty() || EZAFE_STOP.contains(&core(next[0]).as_str()) {
            return Ok(ph);
        }
        let n = split_ws(span).iter().filter(|w| search(r"\w", w)).count().max(1);
        let probe = split_ws(&self.phonemise_marked(&format!("{span} {}", next.join(" ")), true)?).iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let Some(with) = probe.get(n - 1) else { return Ok(ph) };
        let alone = split_ws(&self.phonemise_marked(span, true)?).last().map(|s| s.trim_end_matches('1').to_string()).unwrap_or_default();
        let w = with.trim_end_matches('1');
        let grew = !alone.is_empty() && (w == format!("{alone}e") || w == format!("{alone}ye"));
        if last.ends_with('e') && !span.trim_end().ends_with(['ه', 'ۀ', 'ة']) {
            return Ok(ph);
        }
        if !(with.ends_with('1') && !alone.ends_with('e')) && !grew {
            return Ok(ph);
        }
        let add = if ends_any(&last, &["A", "u", "i", "o", "e"]) { "ye" } else { "e" };
        let mut words: Vec<String> = split_ws(&ph).iter().map(|s| s.to_string()).collect();
        let k = words.len() - 1;
        words[k] = format!("{last}{add}{}", if keep_ezafe { "1" } else { "" });
        Ok(words.join(" "))
    }

    fn base_checked(&self, text: &str) -> R {
        let words: Vec<&str> = split_ws(text).into_iter().filter(|w| search(r"\w", w)).collect();
        let raw = self.base.phonemise(text, true)?;
        let mut ph = if split_ws(&raw).len() > words.len() { collapse_loops(&raw) } else { raw };
        if words.len() >= 2 && words.len() <= 4 && split_ws(&ph).len() < words.len() && words.iter().all(|w| *w == words[0]) {
            let one = collapse_loops(&self.base.phonemise(words[0], true)?);
            if split_ws(&one).len() == 1 { ph = vec![one; words.len()].join(" "); }
        }
        let n = split_ws(&ph).len();
        let nw = words.len();
        if (nw as isize - 1 <= n as isize && n <= nw) || nw < 4 || (n > nw && search(r"[0-9۰-۹٠-٩A-Za-z]", text)) {
            return Ok(ph);
        }
        let mid = nw / 2;
        let cuts: Vec<usize> = (2..nw.saturating_sub(1)).filter(|&k| words[k] == "و").collect();
        let h = cuts.iter().copied().min_by_key(|&k| (k as isize - mid as isize).abs()).unwrap_or(mid);
        let mut left: Vec<String> = if !cuts.is_empty() {
            split_ws(&self.base_checked(&words[..h].join(" "))?).iter().map(|s| s.to_string()).collect()
        } else {
            let mut l: Vec<String> = split_ws(&self.base_checked(&(words[..h].join(" ") + " او"))?).iter().map(|s| s.to_string()).collect();
            l.pop();
            l
        };
        left.extend(split_ws(&self.base_checked(&words[h..].join(" "))?).iter().map(|s| s.to_string()));
        Ok(left.join(" "))
    }

    fn phonemise_marked(&self, text: &str, keep_ezafe: bool) -> R {
        let mut ph = no_ezafe_before_verb(&self.base_checked(text)?);
        let words: Vec<String> = split_ws(text).into_iter().filter(|w| search(&format!("[{FA_LETTER}]"), w)).map(|s| s.to_string()).collect();
        let mut phw: Vec<String> = split_ws(&ph).iter().map(|s| s.to_string()).collect();
        let pairs = align_words(&words, &phw);
        let suspect = |i: usize, j: usize| (len(&phw[j]) >= 3 && !has_vowel(&phw[j])) || !consonants_ok(&words[i], &phw[j]);
        if pairs.iter().any(|&(i, j)| suspect(i, j)) {
            for (i, j) in pairs {
                let no_vowel = len(&phw[j]) >= 3 && !has_vowel(&phw[j]);
                if no_vowel || !consonants_ok(&words[i], &phw[j]) {
                    let alone: Vec<String> = split_ws(&self.base.phonemise(&words[i], true)?).iter().map(|s| s.to_string()).collect();
                    if alone.len() == 1 && has_vowel(&alone[0]) && (no_vowel || consonants_ok(&words[i], &alone[0])) {
                        let a = alone[0].trim_end_matches('1');
                        let ez = if !phw[j].ends_with('1') || alone[0].ends_with('1') { "" }
                            else if phw[j].ends_with("ye1") && a.ends_with(['A', 'u', 'o', 'i', 'e']) { "ye1" }
                            else if phw[j].ends_with("e1") { "e1" } else { "1" };
                        phw[j] = format!("{}{ez}", alone[0]);
                    }
                }
            }
            ph = phw.join(" ");
        }
        let in_fix = |w: &str| lookup(WORD_FIX, w).is_some();
        let pair_fix = |a: &str, b: &str| WORD_FIX_PAIRS.iter().find(|((x, y), _)| *x == a && *y == b).map(|(_, v)| *v);
        if words.iter().any(|w| EZAFE_PRONE.contains(&core(w).as_str())) || words.iter().any(|w| search(KASRA_END, w) || w.contains("ساخت") || w.contains("جستار") || in_fix(&core(w))) {
            let aligned = align_words(&words, &phw);
            let has = |k: usize| aligned.iter().find(|x| x.0 == k).map(|x| x.1);
            for &(i, j) in &aligned {
                let key = core(&words[i]);
                let pair = if i + 1 < words.len() { pair_fix(&key, &core(&words[i + 1])) } else { None };
                if let (Some((a, b)), Some(j2)) = (pair, has(i + 1)) {
                    phw[j] = a.to_string();
                    phw[j2] = b.to_string();
                } else if let Some(fix) = lookup(WORD_FIX, &key) {
                    if i > 0 && pair_fix(&core(&words[i - 1]), &key).is_some() {
                        continue;
                    }
                    let ezafe = phw[j].contains('1') && !NAMES.contains(&fix);
                    phw[j] = if ezafe {
                        match lookup(EZAFE_FORM, fix) {
                            Some(e) => e.to_string(),
                            None => format!("{fix}{}", if ends_any(fix, &["A", "u", "i", "o", "e"]) { "ye1" } else { "e1" }),
                        }
                    } else {
                        fix.to_string()
                    };
                }
            }
            for (i, j) in align_words(&words, &phw) {
                if EZAFE_PRONE.contains(&core(&words[i]).as_str()) && !phw[j].ends_with('1') && !phw[j].ends_with('e') {
                    let punct_after = sub(r"[^\w\u200c]+$", "", &words[i]) != words[i];
                    let next_ok = words.get(i + 1).map_or(false, |n| !EZAFE_STOP.contains(&core(n).as_str()) && !core(n).is_empty());
                    if !punct_after && next_ok {
                        phw[j] = format!("{}e1", phw[j]);
                    }
                }
                if search(KASRA_END, &words[i]) && !phw[j].ends_with('1') {
                    let stripped = sub(r"[^\w]|[ً-ْ]", "", &words[i]);
                    let last = stripped.chars().last();
                    let sound = if matches!(last, Some('ا') | Some('و') | Some('ی') | Some('ه')) { "ye" } else { "e" };
                    let p = phw[j].clone();
                    phw[j] = format!("{}1", if p.ends_with(sound) { p } else { format!("{p}{sound}") });
                }
                if words[i].contains("ساخت") {
                    phw[j] = phw[j].replace("soxt", "sAxt");
                }
                if words[i].contains("جستار") {
                    phw[j] = phw[j].replace("jastAr", "jostAr");
                }
            }
            ph = phw.join(" ");
        }
        ph = split_ws(&ph).iter().map(|w| lookup(PHONEME_FIX, w).unwrap_or(w).to_string()).collect::<Vec<_>>().join(" ");
        ph = split_ws(&ph).iter().map(|w| mikon_fixed(w)).collect::<Vec<_>>().join(" ");
        ph = split_ws(&ph).iter().map(|w| sub(r"^(.+?)e?ye1?\?e?ye(1?)$", r"\1e?e\2", w)).collect::<Vec<_>>().join(" ");
        ph = split_ws(&ph).iter().map(|w| sub(r"^(.+e)\?(aS|at|am)$", r"\1\2", &sub(r"^(.+e)\?i$", r"\1yi", w))).collect::<Vec<_>>().join(" ");
        ph = split_ws(&ph).iter().map(|w| ezafe_after_ain(w)).collect::<Vec<_>>().join(" ");
        ph = sub(r"(?<=[^\s?])allAh(?=e?1?\b)", "ollAh", &ph);
        if text.contains("شها") || text.contains("ش\u{200c}ها") {
            let ws: Vec<String> = split_ws(text).into_iter().filter(|w| search(&format!("[{FA_LETTER}]"), w)).map(|s| s.to_string()).collect();
            let mut pw: Vec<String> = split_ws(&ph).iter().map(|s| s.to_string()).collect();
            for (i, j) in align_words(&ws, &pw) {
                if search("ش\u{200c}?ها(?:ی|یی|یم|یت|یش|یمان|یتان|یشان)?[^\\p{L}]*$", &ws[i]) && !pw[j].contains("ShA") {
                    let ends = ["", "y", "ye", "yi", "yam", "yat", "yaS", "yemAn", "yetAn", "yeSAn"];
                    let at = pw[j].match_indices("SA").map(|(k, _)| k).filter(|&k| ends.contains(&pw[j][k + 2..].trim_end_matches('1'))).last();
                    if let Some(k) = at {
                        pw[j].replace_range(k..k + 2, "ShA");
                    }
                }
            }
            ph = pw.join(" ");
        }
        if !text.replace('\u{200c}', "").contains("سه") {
            ph = ph.replace("sh", "S");
        }
        if !search("[زذضظ]ه", &text.replace('\u{200c}', "")) {
            ph = ph.replace("zh", ";");
        }
        Ok(if keep_ezafe { ph } else { ph.replace('1', "") })
    }
}


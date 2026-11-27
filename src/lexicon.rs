
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
    const VERBS: &[&str] = &[
        // past tense
        "kard", "kardam", "kardi", "kardim", "kardid", "kardand",
        "Sod", "Sodam", "Sodi", "Sodim", "Sodid", "Sodand",
        "bud", "budam", "budi", "budim", "budid", "budand",
        "raft", "raftam", "rafti", "raftim", "raftid", "raftand",
        "dAd", "dAdam", "dAdi", "dAdim", "dAdid", "dAdand",
        "goft", "goftam", "gofti", "goftim", "goftid", "goftand",
        "dASt", "dAStam", "dASti", "dAStim", "dAStid", "dAStand",
        "?Amad", "?Amadam", "?Amadi", "?Amadim", "?Amadid", "?Amadand",
        "mord", "mordam", "mordi", "mordim", "mordid", "mordand",
        "gereft", "gereftam", "gerefti", "gereftim", "gereftid", "gereftand",
        "baxSid", "baxSidam", "baxSidand",
        // past continuous
        "mikard", "mikardam", "mikardand", "nemikard", "nemikardam", "nemikardand",
        "miSod", "miSodam", "miSodand", "nemiSod", "nemiSodam", "nemiSodand",
        "midAd", "midAdam", "midAdand", "nemidAd", "nemidAdam", "nemidAdand",
        "miraft", "miraftam", "miraftand", "nemiraft", "nemiraftam", "nemiraftand",
        "migoft", "migoftam", "migoftand", "nemigoft", "nemigoftam", "nemigoftand",
        "midASt", "midAStam", "midAStand", "nemidASt", "nemidAStam", "nemidAStand",
        // present & subjunctive
        "konad", "konam", "koni", "konim", "konid", "konand",
        "mikonad", "mikonam", "mikoni", "mikonim", "mikonid", "mikonand",
        "nemikonad", "nemikonam", "nemikoni", "nemikonim", "nemikonid", "nemikonand",
        "bokonad", "bokonam", "bokoni", "bokonim", "bokonid", "bokonand",
        "bikonad", "bikonam", "bikoni", "bikonim", "bikonid", "bikonand",
        "dahad", "daham", "dahi", "dahim", "dahid", "dahand",
        "dehad", "deham", "dehi", "dehim", "dehid", "dehand",
        "midahad", "midaham", "midahi", "midahim", "midahid", "midahand",
        "midehad", "mideham", "midehi", "midehim", "midehid", "midehand",
        "nemidahad", "nemidaham", "nemidahi", "nemidahim", "nemidahid", "nemidahand",
        "nemidehad", "nemideham", "nemidehi", "nemidehim", "nemidehid", "nemidehand",
        "bedahad", "bedeham", "bedehi", "bedehim", "bedehid", "bedahand",
        "Savad", "Savam", "Savi", "Savim", "Savid", "Savand",
        "miSavad", "miSavam", "miSavi", "miSavim", "miSavid", "miSavand",
        "nemiSavad", "nemiSavam", "nemiSavi", "nemiSavim", "nemiSavid", "nemiSavand",
        "beSavad", "beSavam", "boSavad",
        "bASad", "bASam", "bASi", "bASim", "bASid", "bASand",
        "nabASad", "nabASam", "nabASi", "nabASim", "nabASid", "nabASand",
        "baxSad", "baxSam", "baxSi", "baxSim", "baxSid", "baxSand",
        "mibaxSad", "mibaxSam", "mibaxSi", "mibaxSim", "mibaxSid", "mibaxSand",
        "nemibaxSad", "nemibaxSam", "nemibaxSi", "nemibaxSim", "nemibaxSid", "nemibaxSand",
        "bebaxSad", "bebaxSam",
        "girad", "giram", "giri", "girim", "girid", "girand",
        "migirad", "migiram", "migirand", "nemigirad", "nemigiram", "nemigirand", "begirad",
        "guyad", "guyam", "guyi", "guyim", "guyid", "guyand",
        "miguyad", "miguyam", "miguyand", "nemiguyad", "nemiguyam", "nemiguyand", "beguyad",
        "dArad", "dAram", "dAri", "dArim", "dArid", "dArand",
        "nadArad", "nadAram", "nadAri", "nadArim", "nadArid", "nadArand",
        "namAyad", "nemAyad", "minamAyad", "minemAyad", "neminamAyad", "neminemAyad",
        "gardad", "gardam", "gardand", "migardad", "migardand", "nemigardad", "nemigardand",
        "?ast", "hast", "nist", "hastand", "nistand", "hastam", "nistam"
    ];
    const NUMBERS: &[&str] = &["yek", "do", "se", "CahAr", "panj", "SeS", "haft", "haSt", "noh", "dah", "yAzdah", "davAzdah", "sizdah",
        "CahArdah", "pAnzdah", "SAnzdah", "hefdah", "hejdah", "nuzdah", "bist", "si", "Cehel", "panjAh", "haftAd", "haStAd", "navad"];
    let mut w: Vec<String> = split_ws(ph).iter().map(|s| s.to_string()).collect();
    for i in 0..w.len().saturating_sub(1) {
        let number = NUMBERS.contains(&w[i].trim_end_matches("e1")) && !NUMBERS.contains(&w[i + 1].as_str()) && w[i + 1] != "?o";
        let next_w = w[i + 1].trim_end_matches('1');
        if (VERBS.contains(&next_w) || number) && w[i].ends_with("e1") && (w[i].chars().count() > 3 || number) {
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

    ("فرداست", "fardAst"),
    ("پیش‌رو", "piSru"),
    ("پیشرو", "piSru"),
    ("صحه", "sehhe"),
    ("لطایف‌الحیلی", "latAyefolhili"),
    ("ارمغان", "?armaqAn"),
    ("متاعی", "matA?i"),
    ("عرضه‌ای", "?arze?i"),
    ("فرضاً", "farzan"),
    ("ابژه‌ی", "?ob;eye"),
    ("ابژه", "?ob;e"),
    ("بینش", "bineS"),
    ("آتی", "?Ati"),
    ("مانوس", "ma?nus"),
    ("رویکردهای", "ruykardhAye"),
    ("حول", "havl"),
    ("موحشی", "muheSi"),
    ("اینده", "?Ayande"),
    ("آینده", "?Ayande"),
    ("کودک‌ازاری", "kudak?AzAri"),
    ("کودک‌آزاری", "kudak?AzAri"),
    ("سلسه‌مراتب", "selselemarAteb"),
    ("سلسله‌مراتب", "selselemarAteb"),
    ("سلسله‌مراتبِ", "selselemarAtebe"),
    ("خانه‌های", "xAnehAye"),
    ("مراقبتی", "morAqebati"),
    ("مبتنی", "mobtani"),
    ("آبزرور", "?Abzerver"),
    ("نشریه‌ی", "naSriyeye"),
    ("جیمی", "jimi"),
    ("سوایل", "sevil"),
    ("شنیع", "Sani?"),
    ("افشای", "?efSAye"),
    ("وست‌مینستر", "vestminster"),
    ("وست مینستر", "vestminster"),
    ("دهشتناک", "dahSatnAk"),
    ("صاحب‌منصبان", "sAhebmansebAn"),
    ("مالیات", "mAliyAt"),
    ("سرانه", "sarAne"),
    ("قضایی", "qazAyi"),
    ("باتلر-اسلاس", "bAtler ?eslAs"),
    ("رُلف", "rolf"),
    ("رلف", "rolf"),
    ("شریرانه‌ای", "SarirAne?i"),
    ("شریرانه", "SarirAne"),
    ("ملتهب", "moltaheb"),
    ("اسلاوی", "?eslAvi"),
    ("ژیژک", ";i;ek"),
    ("نوع", "nov?"),
    ("نادری", "nAderi"),
    ("میل", "meyl"),
    ("معینی", "mo?ayyeni"),
    ("مداراطلب", "modArAtalab"),
    ("میلی", "meyli"),
    ("لیبیدوایی", "libiduvAyi"),
    ("موهن", "muhen"),
    ("موجهی", "movajjehi"),
    ("موجه", "movajjeh"),
    ("زن‌ستیزتر", "zansetiztar"),
    ("طلیعه‌ی", "tali?eye"),
    ("طلیعه", "tali?e"),
    ("شق", "Saqq"),
    ("کیستی", "kisti"),
    ("بی‌ام‌ای", "bi ?em ?Ay"),
    ("بی‌ام ای", "bi ?em ?Ay"),
    ("آمارش", "?AmAraS"),
    ("اوان", "?avAn"),
    ("سهواً", "sahvan"),
    ("بوالهوس", "bolhavas"),
    ("جرم", "jorm"),
    ("شفیلد", "Sefild"),
    ("سوءاستفاده‌ی", "su?estefAdeye"),
    ("اغنیا", "?aqniyA"),
    ("قوادهای", "qavvAdhAye"),
    ("قواد", "qavvAd"),
    ("تن‌هایی", "tanhAyi"),
    ("سرشتی", "sereSti"),
    ("زنانه", "zanAne"),
    ("عرضه‌ی", "?arzeye"),
    ("عرضه", "?arze"),
    ("برمه", "berme"),
    ("بوردیو", "burdiyo"),
    ("کژکاری‌های", "ka;kArihAye"),
    ("کژکاری", "ka;kAri"),
    ("کذب", "kezb"),
    ("بکشد", "bekoSad"),
    ("بکشند", "bekoSand"),
    ("نشویند", "naSuyand"),
    ("نسیان", "nesyAn"),
    ("شن", "Sen"),
    ("جستجو", "jostoju"),
    ("رعب‌انگیزی", "ro?bangizi"),
    ("محرز", "mohraz"),
    ("امتزاجی", "?emtezAji"),
    ("سرخط", "sarxat"),
    ("حمیرا", "homeyrA"),
    ("مشیرزاده", "moSirzAde"),
    ("ماتریالیسم", "mAtriyAlism"),
    ("ماتریالیسمِ", "mAtriyAlisme"),
    ("تاریخیِ", "tArixiye"),
    ("فمینیستی", "feministi"),
    ("فمینیسم", "feminism"),
    ("مشخصاًفمینیستی", "moSaxxasan feministi"),
    ("پیکریافته", "peykaryAfte"),
    ("پیکر‌یافته", "peykaryAfte"),
    ("پایه‌ای", "pAye?i"),
    ("پایهای", "pAye?i"),
    ("پایه", "pAye"),
    ("پایه‌ی", "pAyeye"),
    ("متانظریه‌ای", "metAnazariye?i"),
    ("متانظریهای", "metAnazariyehA"),
    ("متانظریه‌های", "metAnazariyehAye"),
    ("متانظریه‌ها", "metAnazariyehA"),
    ("متانظری", "metAnazari"),
    ("متانظریه", "metAnazariye"),
    ("دعوی", "da?vA"),
    ("دعویِ", "da?vAye"),
    ("تلویحاً", "talvihan"),
    ("تلویحا", "talvihan"),
    ("مشخصاً", "moSaxxasan"),
    ("مشخصا", "moSaxxasan"),
    ("متعهدانه", "mota?ahhedAne"),
    ("متعهدانه‌ی", "mota?ahhedAneye"),
    ("متعهدانهی", "mota?ahhedAneye"),
    ("ذی‌نفعانه", "zinaf?Ane"),
    ("ذینفعانه", "zinaf?Ane"),
    ("ذی‌نفع", "zinaf?"),
    ("ذینفع", "zinaf?"),
    ("حسن نیت", "hosne niyat"),
    ("حسن‌نیت", "hosne niyat"),
    ("کژدیسه", "ka;dise"),
    ("کژدیسی", "ka;disi"),
    ("حسانی", "hessAni"),
    ("حسانیِ", "hessAniye"),
    ("تعین", "ta?ayyon"),
    ("تعیّن", "ta?ayyon"),
    ("تعی", "ta?ayyon"),
    ("صرف", "serf"),
    ("صرفِ", "serfe"),
    ("سلطه", "solte"),
    ("سلطه‌ی", "solteye"),
    ("بسط", "bast"),
    ("بسطِ", "baste"),
    ("وضع", "vaz?"),
    ("وضعِ", "vaz?e"),
    ("موضعی", "mowze?i"),
    ("موضعیِ", "mowze?iye"),
    ("رویکرد", "ruykard"),
    ("رویکردِ", "ruykarde"),
    ("رویکردی", "ruykardi"),
    ("سرشت", "sereSt"),
    ("سرشتِ", "sereSte"),
    ("دیدگاه", "didgAh"),
    ("دیدگاهِ", "didgAhe"),
    ("نگرش", "negareS"),
    ("نگرشِ", "negareSe"),
    ("طبقه", "tabaqe"),
    ("طبقه‌ی", "tabaqeye"),
    ("طبقهی", "tabaqeye"),
    ("طبقات", "tabaqAt"),
    ("مالکان", "mAlekAn"),
    ("مالکانِ", "mAlekAne"),
    ("مالک", "mAlek"),
    ("مالکِ", "mAleke"),
    ("کالا", "kAlA"),
    ("کالاها", "kAlAhA"),
    ("کالاهای", "kAlAhAye"),
    ("انسانی", "?ensAni"),
    ("انسانیِ", "?ensAniye"),
    ("بسطوشرح", "bastoSarh"),
    ("بسط‌وشرح", "bastoSarh"),
    ("اخذکردن", "?axzkardan"),
    ("اخذ", "?axz"),
    ("اخذِ", "?axze"),
    ("توانا کند", "tavAnA konad"),
    ("بسط دهم", "bast daham"),
    ("حکم", "hokm"),
    ("حکمِ", "hokme"),
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
    ("اشکال گوناگون", "?aSkAle gunAgun"),
    ("اشکال گوناگونی", "?aSkAle gunAguni"),
    ("اشکال متفاوت", "?aSkAle motafAvet"),
    ("اشکال متفاوتی", "?aSkAle motafAveti"),
    ("اشکال متنوع", "?aSkAle motanavve?"),
    ("اشکال تازه", "?aSkAle tAze"),
    ("اشکال تازه‌ای", "?aSkAle tAze?i"),
    ("اشکال نوین", "?aSkAle novin"),
    ("اشکال سنتی", "?aSkAle sonnati"),
    ("اشکال اولیه", "?aSkAle ?avvaliyye"),
    ("اشکال پیشین", "?aSkAle piSin"),
    ("اشکال سیاسی", "?aSkAle siyAsi"),
    ("اشکال اجتماعی", "?aSkAle ?ejtemA?i"),
    ("اشکال اقتصادی", "?aSkAle ?eqtesAdi"),
    ("اشکال مبارزه", "?aSkAle mobAreze"),
    ("اشکال اعتراض", "?aSkAle ?e?terAz"),
    ("اشکال اعتراضی", "?aSkAle ?e?terAzi"),
    ("اشکال تولید", "?aSkAle tolid"),
    ("اشکال خشونت", "?aSkAle xoSunat"),
    ("اشکال قدرت", "?aSkAle qodrat"),
    ("اشکال بیان", "?aSkAle bayAn"),
    ("اشکال سازمان‌دهی", "?aSkAle sAzmAndehi"),
    ("اشکال سازماندهی", "?aSkAle sAzmAndehi"),
    ("اشکال خاصی", "?aSkAle xAssi"),
    ("اشکال خاص", "?aSkAle xAss"),
    ("انواع و اشکال", "?anvA? o ?aSkAle"),
    ("به اشکال", "be ?aSkAle"),
    ("در اشکال", "dar ?aSkAle"),
    ("از اشکال", "?az ?aSkAle"),
    ("این اشکال", "?in ?aSkAl"),
    ("همه‌ی اشکال", "hameye ?aSkAle"),
    ("همه اشکال", "hameye ?aSkAle"),
    ("اشکالی ندارد", "?eSkAli nadArad"),
    ("اشکال ندارد", "?eSkAl nadArad"),
    ("اشکالی نیست", "?eSkAli nist"),
    ("بی‌اشکال", "bi?eSkAl"),
    ("بیاشکال", "bi?eSkAl"),
    ("اشکالات", "?eSkAlAt"),
    ("اشکالاتی", "?eSkAlAti"),
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
    ("مبتنی بر", "mobtani bar"),
    ("مبتنی‌بر", "mobtani bar"),
    ("مبتنی", "mobtani"),
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
    ("رای", "ra?y"),
    ("رأی دادن", "ra?y dAdan"),
    ("رای دادن", "ra?y dAdan"),
    ("رأی‌دادن", "ra?y dAdan"),
    ("رای‌دادن", "ra?y dAdan"),
    ("رأی‌گیری", "ra?ygiri"),
    ("رای‌گیری", "ra?ygiri"),
    ("رای گیری", "ra?ygiri"),
    ("رأی‌ها", "ra?yhA"),
    ("رأیها", "ra?yhA"),
    ("رای‌ها", "ra?yhA"),
    ("جبهه", "jebhe"),
    ("جبهه‌ی", "jebheye"),
    ("جبهه ی", "jebheye"),
    ("جبهه‌ی متحد", "jebheye mottahed"),
    ("جبهه متحد", "jebheye mottahed"),
    ("متحد", "mottahed"),
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
    // Standard Persian calendar months & ordinals (all 12 months neutrally covered)
    ("فروردین‌ماه", "farvardinmAh"),
    ("اردیبهشت‌ماه", "?ordibeheStmAh"),
    ("خرداد‌ماه", "xordAdmAh"),
    ("خردادماه", "xordAdmAh"),
    ("تیر‌ماه", "tirmAh"),
    ("تیرماه", "tirmAh"),
    ("مرداد‌ماه", "mordAdmAh"),
    ("مردادماه", "mordAdmAh"),
    ("شهریور‌ماه", "SahrivarmAh"),
    ("شهریورماه", "SahrivarmAh"),
    ("مهر‌ماه", "mehrmAh"),
    ("مهرماه", "mehrmAh"),
    ("مهر ماه", "mehrmAh"),
    ("آبان‌ماه", "?AbAnmAh"),
    ("آذر‌ماه", "?AzarmAh"),
    ("آذرماه", "?AzarmAh"),
    ("آذر ماه", "?AzarmAh"),
    ("ماه آذر", "mAhe ?Azar"),
    ("ماه مهر", "mAhe mehr"),
    ("ماه دی", "mAhe dey"),
    ("دی‌ماه", "deymAh"),
    ("دی ماه", "deymAh"),
    ("دیماه", "deymAh"),
    ("دی", "dey"),
    ("بهمن‌ماه", "bahmanmAh"),
    ("اسفند‌ماه", "?esfandmAh"),
    ("اسفندماه", "?esfandmAh"),
    ("یکم", "yekom"),
    ("یکمِ", "yekome"),
    // General everyday Persian dictionary & homograph fixes
    ("اصناف", "?asnAf"),
    ("برش", "boreS"),
    ("برش‌ها", "boreShA"),
    ("برشها", "boreShA"),
    ("برش‌های", "boreShAye"),
    ("برشی", "boreSi"),
    ("دو برش", "do boreS"),
    ("دوبرش", "do boreS"),
    ("بحث", "bahs"),
    ("بحث‌ها", "bahshA"),
    ("بحثها", "bahshA"),
    ("بحث‌های", "bahshAye"),
    ("بحثهای", "bahshAye"),
    ("بحثی", "bahsi"),
    ("بی‌تأمل", "bita?ammol"),
    ("بی‌تأملی", "bita?ammoli"),
    ("بی‌تامل", "bita?ammol"),
    ("بی‌تاملی", "bita?ammoli"),
    ("بی‌رمق", "biramaq"),
    ("بی‌واسطه", "bivAsete"),
    ("پدرسالار", "pedarsAlAr"),
    ("پدرسالاری", "pedarsAlAri"),
    ("پیشبرد", "piSbord"),
    ("پیش‌برد", "piSbord"),
    ("پیشگیری", "piSgiri"),
    ("پیش‌گیری", "piSgiri"),
    ("پیگرد", "peygard"),
    ("پی‌گرد", "peygard"),
    ("تأمل", "ta?ammol"),
    ("تامل", "ta?ammol"),
    ("تشخیص", "taSxis"),
    ("تشخیصی", "taSxisi"),
    ("تنگ‌دست", "tangdast"),
    ("تنگ‌دستان", "tangdastAn"),
    ("تهی‌دست", "tohidast"),
    ("تهی‌دستان", "tohidastAn"),
    ("تهی‌دستی", "tohidasti"),
    ("جوّ", "javv"),
    ("جوِ", "javve"),
    ("جوّی", "javvi"),
    ("جوسازی", "javvsAzi"),
    ("خرد جمعی", "xerade jam?i"),
    ("خرد", "xerad"),
    ("خردی", "xeradi"),
    ("بی‌خرد", "bixerad"),
    ("بی خرد", "bixerad"),
    ("بیخرد", "bixerad"),
    ("بی‌خردی", "bixeradi"),
    ("بی‌خردان", "bixeradAn"),
    ("کم‌خرد", "kamxerad"),
    ("کم خرد", "kamxerad"),
    ("کمخرد", "kamxerad"),
    ("کم‌خردی", "kamxeradi"),
    ("خردورز", "xeradvarz"),
    ("خردورزی", "xeradvarzi"),
    ("خردورزان", "xeradvarzAn"),
    ("خردگرا", "xeradgarA"),
    ("خردگرایی", "xeradgerAyi"),
    ("خردگرایان", "xeradgarAyAn"),
    ("خردگرایانه", "xeradgarAyAne"),
    ("خردستیز", "xeradsetiz"),
    ("خردستیزی", "xeradsetizi"),
    ("خردپیشه", "xeradpiSe"),
    ("خردمندان", "xeradmandAn"),
    ("خردمند", "xeradmand"),
    ("خردمندانه", "xeradmandAne"),
    ("خرد و کلان", "xord ?o kalAn"),
    ("خرد و بزرگ", "xord ?o bozorg"),
    ("خرد و ریز", "xord ?o riz"),
    ("ریز و خرد", "riz ?o xord"),
    ("پول خرد", "pule xord"),
    ("سنگ خرد", "sange xord"),
    ("کاسب خرد", "kAsebe xord"),
    ("مالکیت خرد", "mAlekiyyate xord"),
    ("تولید خرد", "tolide xord"),
    ("سرمایه خرد", "sarmAyeye xord"),
    ("سرمایه‌ی خرد", "sarmAyeye xord"),
    ("مقیاس خرد", "meqyAse xord"),
    ("اقتصاد خرد", "?eqtesAde xord"),
    ("خرد شد", "xord Sod"),
    ("خرد شدند", "xord Sodand"),
    ("خرد شده", "xord Sode"),
    ("خرد شدن", "xord Sodan"),
    ("خرد شود", "xord Savad"),
    ("خرد می‌شود", "xord miSavad"),
    ("خرد کرد", "xord kard"),
    ("خرد کردند", "xord kardand"),
    ("خرد کرده", "xord karde"),
    ("خرد کردن", "xord kardan"),
    ("خرد کند", "xord konad"),
    ("خرد می‌کند", "xord mikonad"),
    ("خرده‌فرهنگ", "xordefarhang"),
    ("خرده فرهنگ", "xordefarhang"),
    ("خرده‌بورژوازی", "xordebur;vAzi"),
    ("خرده بورژوازی", "xordebur;vAzi"),
    ("خرده‌فروشی", "xordeforuSi"),
    ("خرده فروشی", "xordeforuSi"),
    ("خرده‌فروش", "xordeforuS"),
    ("خرده‌پا", "xordepA"),
    ("خرده پا", "xordepA"),
    ("خرده‌گیری", "xordegiri"),
    ("خرده گیری", "xordegiri"),
    ("خرده‌مالک", "xordemAlek"),
    ("نمی‌خرد", "nemixarad"),
    ("نخرد", "naxarad"),
    ("خلط", "xalt"),
    ("خلط مبحث", "xalte mabhas"),
    ("خودتشخیصی", "xodtaSxisi"),
    ("خود‌تشخیصی", "xodtaSxisi"),
    ("رمق", "ramaq"),
    ("روند", "ravand"),
    ("روندها", "ravandhA"),
    ("روند‌ها", "ravandhA"),
    ("روندهای", "ravandhAye"),
    ("روندی", "ravandi"),
    ("زن‌ستیز", "zansetiz"),
    ("زن‌ستیزی", "zansetizi"),
    ("سرخ‌پوست", "sorxpust"),
    ("سرخ‌پوستان", "sorxpustAn"),
    ("سفیدپوست", "sefidpust"),
    ("سفید‌پوست", "sefidpust"),
    ("سفید پوست", "sefidpust"),
    ("سفیدپوستان", "sefidpustAn"),
    ("سفید‌پوستان", "sefidpustAn"),
    ("سیاه‌پوست", "siyAhpust"),
    ("سیاهپوست", "siyAhpust"),
    ("سیاه پوست", "siyAhpust"),
    ("سیاه‌پوستان", "siyAhpustAn"),
    ("سیاهپوستان", "siyAhpustAn"),
    ("شکل‌گیر", "Seklgir"),
    ("شکل‌گیری", "Seklgiri"),
    ("صنف", "senf"),
    ("صنف‌ها", "senfhA"),
    ("صنفها", "senfhA"),
    ("صنفی", "senfi"),
    ("صنفی‌ها", "senfihA"),
    ("ظرفیت", "zarfiyyat"),
    ("ظرفیت‌ها", "zarfiyyathA"),
    ("ظرفیتها", "zarfiyyathA"),
    ("ظرفیت‌های", "zarfiyyathAye"),
    ("ظرفیتی", "zarfiyyati"),
    ("علی‌الاصول", "?alal?osul"),
    ("علی‌الحساب", "?alalhesAb"),
    ("علی الحساب", "?alalhesAb"),
    ("علی‌الظاهر", "?alazzAher"),
    ("علی‌القاعده", "?alalqa?ede"),
    ("علی‌ای‌حال", "?alA ?ayye hAl"),
    ("علی ای حال", "?alA ?ayye hAl"),
    ("علی‌رغم", "?alA raqme"),
    ("علی رغم", "?alA raqme"),
    ("علی", "?ali"),
    ("فرافکن", "farAfkan"),
    ("فرافکنی", "farAfkani"),
    ("قاضی", "qAzi"),
    ("قضات", "qozzAt"),
    ("کلاهبرداری", "kolAhbardAri"),
    ("کلاه‌برداری", "kolAhbardAri"),
    ("مردسالار", "mardsAlAr"),
    ("مردسالاری", "mardsAlAri"),
    ("مصرانه", "moserrAne"),
    ("معین", "mo?ayyan"),
    ("معینی", "mo?ayyani"),
    ("می‌خرد", "mixarad"),
    ("میخرد", "mixarad"),
    ("بخرد", "bexarad"),
    ("می‌رود", "miravad"),
    ("میرود", "miravad"),
    ("نمی‌رود", "nemiravad"),
    ("برود", "beravad"),
    ("نرود", "naravad"),
    ("به‌کار رود", "be kAr ravad"),
    ("به کار رود", "be kAr ravad"),
    ("به‌شمار رود", "be SomAr ravad"),
    ("به شمار رود", "be SomAr ravad"),
    ("پیش رود", "piS ravad"),
    ("از میان رود", "?az miyAn ravad"),
    ("از بین رود", "?az beyn ravad"),
    ("گمان رود", "gomAn ravad"),
    ("انتظار رود", "?entezAr ravad"),
    ("نابرابر", "nAbarAbar"),
    ("نابرابری", "nAbarAbari"),
    ("نابرابری‌ها", "nAbarAbarihA"),
    ("نابرابریها", "nAbarAbarihA"),
    ("واسط", "vAset"),
    ("واسطه", "vAsete"),
    ("ویژه", "vi;e"),
    ("ویژه‌ای", "vi;e?i"),
    ("ویژه ای", "vi;e?i"),
    ("ویژهای", "vi;e?i"),
    ("به‌ویژه", "bevi;e"),
    ("به ویژه", "bevi;e"),
    ("ویژگی", "vi;egi"),
    ("ویژگی‌ها", "vi;egihA"),
    ("ویژگیها", "vi;egihA"),
    ("ویژگی‌های", "vi;egihAye"),
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
    ("مضحک", "mozhek"),
    ("حول", "hol"),
    ("حول و حوش", "hol ?o hoS"),
    ("کردار", "kerdAr"),
    ("کردار نیک", "kerdAre nik"),
    ("موطن", "motan"),
    ("بنگریم", "bengarim"),
    ("ممارست", "momAresat"),
    ("جبلی", "jebelli"),
    ("ادعیه", "?ad?iye"),
    ("اینده", "?Ayande"),
    ("گوتیک", "gotik"),
    ("پژوهش‌گران", "pa;uheSgarAn"),
    ("ماکیان", "mAkiyAn"),
    ("علیرغم", "?alA raqme"),
    ("سحر و جادو", "sehr ?o jAdu"),
    ("جادو و سحر", "jAdu va sehr"),
    ("سحرآمیز", "sehr?Amiz"),
    ("سحر آمیز", "sehr?Amiz"),
    ("باطل‌السحر", "bAtelossehr"),
    ("سحر", "sahar"),
    ("فقدان", "foqdAn"),
    ("پرسمان", "porsmAn"),
    ("متهم", "mottahem"),
    ("متهمان", "mottahemAn"),
    ("محتمل", "mohtamal"),
    ("متداول", "motadAvel"),
    ("کشت و زرع", "keSt ?o zar?"),
    ("کشت و کار", "keSt ?o kAr"),
    ("کشتزار", "keStzAr"),
    ("کشتزارها", "keStzArhA"),
    ("شترمرغ", "Sotormorq"),
    ("شتر مرغ", "Sotormorq"),
    ("کرم ابریشم", "kerme ?abriSam"),
    ("کرم خاکی", "kerme xAki"),
    ("پلی از", "poli ?az"),
    ("گرد دلاور", "gorde delAvar"),
    ("بعد اول", "bo?de ?avval"),
    ("بعد دوم", "bo?de dovvom"),
    ("بعد سوم", "bo?de sevvom"),
    ("بعد چهارم", "bo?de CahArom"),
    ("بعد سیاسی", "bo?de siyAsi"),
    ("بعد اجتماعی", "bo?de ?ejtemA?i"),
    ("بعد اقتصادی", "bo?de ?eqtesAdi"),
    ("بعد فرهنگی", "bo?de farhangi"),
    ("بعد روانی", "bo?de ravAni"),
    ("بعد فلسفی", "bo?de falsafi"),
    ("بعد نظری", "bo?de nazari"),
    ("بعد عملی", "bo?de ?amali"),
    ("تک‌بعدی", "takbo?di"),
    ("تک بعدی", "takbo?di"),
    ("دوبعدی", "dobo?di"),
    ("دو بعدی", "dobo?di"),
    ("سه‌بعدی", "sebo?di"),
    ("سه بعدی", "sebo?di"),
    ("چندبعدی", "Candbo?di"),
    ("چند بعدی", "Candbo?di"),
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

    pub fn spelling_phonemes(&self, spelling: &str) -> R {
        let spelling = strip(spelling);
        if spelling.is_empty() {
            return Ok(String::new());
        }
        // 1) User-typed Finglish pronunciation (e.g. "mobtani", "boresh", "shekl giri")
        if crate::phon::is_plain_finglish_spelling(spelling) {
            return Ok(crate::phon::finglish_to_phonemes(spelling));
        }
        // 2) Built-in internal Parsigo phoneme code (contains uppercase A/S/C/?/;)
        if search(PHONEME_WORD, &spelling.replace(' ', "")) {
            return Ok(pronounce(spelling));
        }
        // 3) Persian spelling WITH vowel marks (َ ِ ُ ْ ّ): merge user's explicit marks onto base G2P
        //    so marking even 1 letter (e.g. "بُرش" or "مُبتنی" or "اَشکال") keeps all other vowels natural!
        if search(r"[ً-ْ]", spelling) {
            let mut out_words = Vec::new();
            for w in split_ws(spelling) {
                let plain: String = w.chars().filter(|c| !('\u{064b}'..='\u{0652}').contains(c) && *c != '\u{0670}').collect();
                let base_ph = if !plain.is_empty() {
                    self.base.phonemise(&repair_default(&plain), true).unwrap_or_default()
                } else {
                    String::new()
                };
                let base_one = split_ws(&base_ph).into_iter().next().unwrap_or("");
                let merged = if !base_one.is_empty() {
                    crate::phon::apply_user_marks(w, base_one)
                } else {
                    pronounce_word(w)
                };
                if !merged.is_empty() {
                    out_words.push(merged);
                }
            }
            if !out_words.is_empty() {
                return Ok(out_words.join(" "));
            }
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
        let mut all: Vec<(String, String, bool)> =
            BUILTIN_LEXICON.iter().chain(GENERAL_LEXICON).chain(RUST_LEXICON).map(|(a, b)| (a.to_string(), b.to_string(), true)).collect();
        for (w, s) in user {
            let clean_w = repair_default(w.trim().trim_matches(|c: char| "«»()[]{}،؛:.!?؟\"'-–—".contains(c)));
            if clean_w.is_empty() {
                continue;
            }
            match all.iter_mut().find(|x| repair_default(&x.0) == clean_w) {
                Some(x) => {
                    x.0 = clean_w;
                    x.1 = s.clone();
                    x.2 = false;
                }
                None => all.push((clean_w, s.clone(), false)),
            }
        }
        let mut lex: Vec<(String, String)> = Vec::new();
        for (word, spelling, builtin) in all {
            let word = repair_default(&word);
            let ph = if builtin { pronounce(&spelling) } else { self.spelling_phonemes(&spelling).unwrap_or_default() };
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
        let mode = if crate::vocab::is_finglish() { "fg|" } else if source_is_dutch() { "nl|" } else { "" };
        let key = (if mode.is_empty() { text.to_string() } else { format!("{mode}{text}") }, keep_ezafe);
        if let Some(p) = self.memo.lock().unwrap().get(&key) {
            return Ok(p.clone());
        }
        let r = self.phonemise_uncached(text, keep_ezafe)?;
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
        // Fix floating kasra (ezafe) preceded by space (e.g. "سلسه‌مراتب ِ" -> "سلسه‌مراتبِ", "حسانی ِ" -> "حسانیِ", "بسط‌وشرح ِ" -> "بسط‌وشرحِ")
        text = sub(r"(?<=[ء-غف-يپچژکگی])\s+ِ", "ِ", &text);
        // Fix tanwin glued directly to following Persian word (e.g. "مشخصاًفمینیستی" -> "مشخصاً فمینیستی")
        text = sub(r"(?<=[\u0600-\u06FF]اً)(?=[\u0600-\u06FF])", " ", &text);
        // Fix common OCR fused or split words
        text = sub(r"(?<![ء-غف-يپچژکگی\u200c])بسط[\u200c]?و[\u200c]?شرح(?![ء-غف-يپچژکگی\u200c])", "بسط و شرح", &text);
        text = sub(r"(?<![ء-غف-يپچژکگی\u200c])ح\s+کم(?![ء-غف-يپچژکگی\u200c])", "حکم", &text);
        text = sub(r"(?<![ء-غف-يپچژکگی\u200c])تعی\s+(?=(?:نمی‌|نمی\s+|می‌|می\s+)?بخش)", "تعین ", &text);
        text = sub(r"(?<![ء-غف-يپچژکگی\u200c])اخذکردن", "اخذ کردن", &text);
        text = sub(r"(?<![ء-غف-يپچژکگی\u200c])پایهای(?![ء-غف-يپچژکگی\u200c])", "پایه\u{200c}ای", &text);
        text = sub(r"(?<=[\u0600-\u06FF][^اآو]ه)ی(?![ء-غف-يپچژکگی\u200c])", "\u{200c}ی", &text);
        text = sub(r"(?<![ء-غف-يپچژکگی\u200c])پیکریافته", "پیکر\u{200c}یافته", &text);
        text = sub(r"(?<![ء-غف-يپچژکگی\u200c])متانظری", "متا\u{200c}نظری", &text);
        // General Persian compound ZWNJ boundary rules (teaches G2P thousands of Persian compounds cleanly)
        text = sub(
            r"([\u0600-\u06FF]{2,})[\s\u200c]?(سالار(?:ی|ان|انه)?|ستیز(?:ی|ان|انه)?|بگیر(?:ان|ها|‌ها|ی)?|بردار(?:ی|ان|ها|‌ها)?|تشخیصی)(?![\u0600-\u06FF\u200c])",
            "\\1\u{200c}\\2",
            &text,
        );
        text = sub(
            r"(?<![\u0600-\u06FF\u200c])(بی|خود|پیش|تهی|تنگ)[\s\u200c]?(تأمل(?:ی)?|تامل(?:ی)?|واسطه|رمق|دست(?:ان|ی)?|گیری|برد)(?![\u0600-\u06FF\u200c])",
            "\\1\u{200c}\\2",
            &text,
        );
        // General all-12 Persian calendar months + year rule (e.g. "مهر ۵۷", "آذر ۱۳۹۸", "دی ۱۴۰۲")
        text = sub(
            r"(?<![\u0600-\u06FF\u200c])(فروردین|اردیبهشت|خرداد|تیر|مرداد|شهریور|مهر|آبان|آذر|دی|بهمن|اسفند)\s+(?=[\d۰-۹٠-٩]{2,4}(?![\d۰-۹٠-٩]))",
            "\\1\u{200c}ماه ",
            &text,
        );
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
        let has_harakat = |w: &str| {
            let trimmed = w.trim_end_matches(|c: char| !"آابپتثجچحخدذرزژسشصضطظعغفقکگلمنوهیيئؤأإةۀ\u{064b}\u{064c}\u{064d}\u{064e}\u{064f}\u{0650}\u{0651}\u{0652}".contains(c));
            let without_ezafe_kasra = trimmed.strip_suffix('\u{0650}').unwrap_or(trimmed);
            without_ezafe_kasra.chars().any(|c| ('\u{064b}'..='\u{0652}').contains(&c))
        };
        let pair_fix = |a: &str, b: &str| WORD_FIX_PAIRS.iter().find(|((x, y), _)| *x == a && *y == b).map(|(_, v)| *v);
        if words.iter().any(|w| EZAFE_PRONE.contains(&core(w).as_str())) || words.iter().any(|w| has_harakat(w) || search(KASRA_END, w) || w.contains("ساخت") || w.contains("جستار") || in_fix(&core(w))) {
            let aligned = align_words(&words, &phw);
            let has = |k: usize| aligned.iter().find(|x| x.0 == k).map(|x| x.1);
            for &(i, j) in &aligned {
                if has_harakat(&words[i]) {
                    let clean_w = words[i].trim_matches(|c: char| "«»()[]{}،؛:.!?؟\"'-–—".contains(c));
                    let ez_kasra = search(KASRA_END, clean_w);
                    let w_for_marks = if ez_kasra { clean_w.trim_end_matches('\u{0650}') } else { clean_w };
                    let had_ez = phw[j].ends_with('1') || ez_kasra;
                    let base = if let Some(b) = phw[j].strip_suffix("ye1") {
                        b
                    } else if let Some(b) = phw[j].strip_suffix("e1") {
                        b
                    } else {
                        phw[j].trim_end_matches('1')
                    };
                    let marked = crate::phon::apply_user_marks(w_for_marks, base);
                    if !marked.is_empty() {
                        phw[j] = if had_ez && !marked.ends_with('1') {
                            let suf = if ends_any(&marked, &["A", "u", "i", "o", "e"]) { "ye1" } else { "e1" };
                            format!("{marked}{suf}")
                        } else {
                            marked
                        };
                        continue;
                    }
                }
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


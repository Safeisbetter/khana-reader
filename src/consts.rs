#![allow(dead_code)]

pub const RTL_RE: &str = "[\u{590}-\u{8ff}יִ-\u{fdff}ﹰ-\u{feff}]";
pub const LTR_RUN_RE: &str = "[A-Za-z0-9۰-۹٠-٩.,:/%\\-+@_]+";
pub const MIRROR: &[(char, &str)] = &[('(', ")"), (')', "("), ('[', "]"), (']', "["), ('{', "}"), ('}', "{"), ('«', "»"), ('»', "«")];
pub const BRACKETS: &[(char, &str)] = &[('(', ")"), (')', "("), ('[', "]"), (']', "[")];
pub const YEH_KAF: &[(char, &str)] = &[('ي', "ی"), ('ى', "ی"), ('ك', "ک")];
pub const HEH: &[(char, &str)] = &[('ھ', "ه"), ('ہ', "ه")];
pub const FA_LETTER: &str = "ء-غف-يپچژکگی";
pub const MI_GLUED: &str = "(?<![ء-غف-يپچژکگی\u{200c}])(ن?می)(?=(?:(?:کن|شو|ده|گیر|گوی|ساز|آور|یاب|رس|خواه|باش|گذار|کش|نویس|خوان|رو|گرد|ماند|پرداز|پذیر|خور|گذر|ریز|خیز|بین|شناس|پسند|فرست|گشا|سپار)(?:م|ی|د|یم|ید|ند)|دان(?:م|د|یم|ید|ند)|توان(?:م|ی|د|یم|ید|ند)?|آی(?:م|ی|د|یم|ید|ند)|بایست(?:ی)?|(?:کرد|شد|داد|گرفت|گفت|یافت|آمد|رفت|دید|برد|خورد|گذشت|ساخت|پرداخت|نوشت|خواند|دانست|توانست|خواست|آورد|افتاد|ایستاد|رسید|گشت|گردید|نمود|شناخت|پذیرفت|ماند)(?:م|ی|یم|ید|ند)?)(?![ء-غف-يپچژکگی\u{200c}]))";
pub const MI_PREFIX: &str = "(?<![ء-غف-يپچژکگی\u{200c}])(ن?می) (?=[ء-غف-يپچژکگی])";
pub const HA_SUFFIX: &str = "(?<=[ء-غف-يپچژکگی]) (ها|های|هایی|هایم|هایت|هایش|هایمان|هایتان|هایشان|شان|تان|مان|تر|تری|ترین)(?![ء-غف-يپچژکگی])|(?<=ه) (ی)(?![ء-غف-يپچژکگی])";
pub const SPLIT_WORDS: &str = "(?<![ء-غف-يپچژکگی\\u200c])(بل که|هم چنین|هم چون|چنان چه|هر گز|آن گاه|هیچ گاه|پی ریزی|روی گردان|آن قدر|ریشه کن|فرمان برداری)(?![ء-غف-يپچژکگی\\u200c])";
pub const JOINED: &[(&str, &str)] = &[
    ("بل که", "بلکه"),
    ("هم چنین", "همچنین"),
    ("هم چون", "همچون"),
    ("چنان چه", "چنانچه"),
    ("هر گز", "هرگز"),
    ("آن گاه", "آنگاه"),
    ("هیچ گاه", "هیچگاه"),
    ("پی ریزی", "پی\u{200c}ریزی"),
    ("روی گردان", "روی\u{200c}گردان"),
    ("آن قدر", "آنقدر"),
    ("ریشه کن", "ریشه\u{200c}کن"),
    ("فرمان برداری", "فرمانبرداری"),
];
pub const PREFIXED_VERB: &str = "(?<![\\w\\u200c])(فرومی|برمی|درمی|وامی|فرانمی|برنمی) (?=[\\w])";
pub const WORD_SUFFIX: &str = "(?<=[ء-غف-يپچژکگی]{3}) (گر|گری|گرها|گرانه|گرا|گرایی|گرایان|گرایانه|بینانه|مند|مندی|مندان|مندانه)(?![ء-غف-يپچژکگی])";
pub const FOOTNOTE_REF: &str = "\\s?[(«»]*[\\[\\]]\\s*ء?\\s*[\\d۰-۹٠-٩][\\d۰-۹٠-٩A-Za-z]{0,2}\\s*[\\[\\]«»()][)«»]*";
pub const PAGE_REF: &str = "(?<![\\w])[\\d۰-۹٠-٩]{1,3}\\s*[.\\-–]\\s*(?:صفحه\\u200c?[ءی]?|صفحات|ص\\.?)(?:\\s*[\\d۰-۹٠-٩]+(?:\\s*[-–]\\s*[\\d۰-۹٠-٩]+)?|\\s+\\S(?=\\s|$))?\\s*[؟?.]?(?![\\w])";
pub const CITATION: &str = "[A-Za-zÀ-ÿ][^\\u0600-\\u06FF]*?(?:op\\.\\s?cit|ibid|Ibid|pp?\\.\\s?[\\dxivlcXIVLC])[^\\u0600-\\u06FF]*";
pub const LTR_STRONG: &str = "A-Za-z0-9۰-۹٠-٩";
pub const LTR_RUN_IN_RTL: &str = "[A-Za-z0-9۰-۹٠-٩](?:[A-Za-z0-9۰-۹٠-٩ .,:/\\-%+_@'’]*[A-Za-z0-9۰-۹٠-٩])?";
pub const RTL_LETTER_RE: &str = "[ء-يپ-ۓﭐ-\u{fdff}ﹰ-\u{feff}A-Za-z]";
pub const LIG_LETTERS: &str = "اآبپتثجچحخدذرزژسشصضطظعغفقکگلمنوهی";
pub const LIG_FA: &str = "[\\u0621-\\u063A\\u0641-\\u064A\\u067E\\u0686\\u0698\\u06A9\\u06AF\\u06CC\\u0622]";
pub const CITATION_LINE: &str = "\\((?:19|20)\\d\\d[a-z]?\\)|\\b(?:19|20)\\d\\d[).,]|\\bet al\\b|\\bsupra\\b|\\b[Ii]bid\\b|\\bpp?\\.\\s?\\d|\\bnote \\d|\\bPress\\b|\\bJournal\\b|\\bvol\\.|\\beds?\\.|Review|University|https?://|www\\.";
pub const NOTES_HEAD: &str = "^\\s*(?:یادداشت\\u200c?ها|پی\\u200c?نوشت\\u200c?ها|پانویس\\u200c?ها|پانوشت\\u200c?ها|منابع(?: و مآخذ)?|کتاب\\u200c?نامه|توضیحات|Notes|NOTES|References|Bibliography)\\s*[:.]?\\s*$";
pub const NOTE_LABEL: &str = "^\\s*[\\[\\]]\\s*[\\d۰-۹٠-٩]{1,3}\\s*[\\[\\]]";
pub const NOTE_ITEM: &str = "^\\s*(?:[\\d۰-۹٠-٩]{1,3}\\s*[.\\-)]|[\\d۰-۹٠-٩]{1,3}\\s*$)";
pub const OCR_LANGS: &[&str] = &["ar-SA", "en-US", "nl-NL"];
pub const OCR_SWAP: &[(&str, &str)] = &[
    ("ج", "جچ"),
    ("ک", "کگ"),
    ("ز", "زژ"),
    ("ث", "ثژ"),
    ("ب", "بپ"),
];
pub const OCR_CONFUSE: &[(&str, &str)] = &[
    ("ث", "د"),
    ("ن", "دت"),
    ("د", "نذ"),
    ("ذ", "د"),
    ("ب", "ن"),
    ("ه", "ب"),
    ("ت", "ن"),
    ("ی", "پب"),
    ("ر", "ژز"),
    ("گ", "ک"),
    ("ا", "آ"),
];
pub const OCR_GLUED: &[(&str, &str)] = &[

];
pub const COMPOUND_PAIRS: &[(&str, &str)] = &[
    ("روش|شناخت|جامعه|مردم|زبان|زیست|ریشه|معرفت|باستان|ستاره|پدیدار|هستی|روان", "شناس\\p{L}*"),
    ("مفصل", "بندی"),
    ("روان", "کاوی|کاوان"),
    ("حکم", "رانی"),
    ("فرمان", "روایی"),
    ("پیش", "گفتار"),
    ("مشروعیت", "بخشی"),
    ("قانون", "مند\\p{L}*"),
    ("برده", "داری"),
    ("تاریخ", "نگار\\p{L}*"),
    ("نظریه", "پرداز\\p{L}*"),
    ("تجربه", "گرا\\p{L}*"),
    ("خود", "تصدیقی"),
];

pub const OCR_MENDED: &[(&str, &str)] = &[

];

pub const OCR_PHRASES: &[(&str, &str)] = &[

];
pub const OCR_SHORT_WORDS: &[&str] = &["آن", "از", "او", "اگر", "با", "بر", "به", "بی", "تا", "تو", "در", "دو", "را", "سر", "سه", "ما", "من", "می", "نه", "نی", "هر", "هم", "و", "پا", "پس", "چه", "که", "یا", "یک"];
pub const OCR_PREFER: &[&str] = &["اگر", "بزرگ", "دیگر", "مگر", "نثار", "هرگز", "چرا", "چشم", "چنان", "چند", "چنین", "چه", "چهار", "چون", "چگونه", "چیز", "ژرف", "گاه", "گروه", "گرچه", "گفت", "گفتند", "گونه"];
pub const EN_WORDS: &[&str] = &["and", "are", "as", "be", "but", "by", "for", "from", "have", "her", "his", "it", "not", "of", "on", "or", "that", "the", "they", "this", "to", "was", "which", "with", "you"];
pub const NL_WORDS: &[&str] = &["aan", "als", "bij", "dat", "de", "die", "een", "en", "er", "het", "maar", "met", "naar", "niet", "nog", "ook", "op", "te", "uit", "van", "voor", "wel", "wordt", "zich", "zijn"];
pub const ABBREV: &str = "(?i)\\b(?:Mr|Mrs|Ms|Dr|Prof|St|vs|etc|e\\.g|i\\.e|cf|al|Fig|No|pp?|vol|ed|eds|dhr|mevr|bijv|o\\.a|d\\.w\\.z|nl|ca|blz|jhr)\\.$";
pub const PHONEME_FIX: &[(&str, &str)] = &[
    ("?omdat", "?omdatan"),
    ("?eynA", "?eynan"),
    ("tadrijA", "tadrijan"),
    ("serAhatA", "sarAhatan"),
    ("rasmA", "rasman"),
    ("zAtA", "zAtan"),
    ("joz?A", "joz?an"),
    ("SodidA", "Sadidan"),
    ("sarihA", "sarihan"),
    ("mostaqlA", "mostaqellan"),
    ("manteqA", "manteqan"),
    ("teluyhA", "talvihan"),
    ("kolA", "kollan"),
    ("maSur", "maShur"),
    ("nafi", "nafy"),
    ("fominism", "faminism"),
    (";emnan", "zemnan"),
    (";emni", "zemni"),
    (";emneye1", "zemneye1"),
    ("beduneye1", "bedune1"),
    ("beduneye", "bedune"),
    ("jaz", "joz"),
    ("beh", "be"),
    ("derham", "darham"),
    ("derham1", "darham1"),
    ("bannA", "banA"),
    ("bannAye1", "banAye1"),
    ("bah", "be"),
    ("mobtani", "mabtani"),
    ("mobtani1", "mabtani1"),
    ("mian", "miyAn"),
    ("oftan", "goftan"),
    ("mioftad", "miyoftad"),
    ("sent", "sonnat"),
    ("sennet", "sonnat"),
    ("sennete1", "sonnate1"),
    ("darak", "dark"),
    ("darake1", "darke1"),
    ("beyAband", "beyAband"),
    ("biahammiyat", "bi?ahammiyat"),
    ("?extifAye", "?extefAye"),
    ("?extifA", "?extefA"),
    ("qaviyyan", "qavviyan"),
    ("nazad", "nazd"),
    ("nazade1", "nazde1"),
    ("?estehaqqAqAte1", "?estehqAqAte1"),
    ("sat?he1", "sathe1"),
    ("sat?h", "sath"),
    ("qesri", "qasri"),
    ("kadgozAri", "kodgozAri"),
    ("mijavad", "mijuyad"),
    ("lafzA", "lafzan"),
    ("?ejmAlA", "?ejmAlan"),
    ("motAleye1?Ate", "motAle?Ate1"),
    ("barpAyeye1", "barpAye1"),
    ("goftamAni", "goftemAni"),
    ("goftamAn", "goftemAn"),
    ("goftamAne1", "goftemAne1"),
    ("naqSAye", "naqShAye"),
    ("naqSAye1", "naqShAye1"),
    ("beonvAne1", "be?onvAne1"),
    ("beonvAn", "be?onvAn"),
    ("xodaS", "xodeS"),
    ("royAro", "ruyAru"),
    ("xviStanemAn", "xiStanemAn"),
    ("xviStan", "xiStan"),
    ("tafraqeafkan", "tafraqe?afkan"),
    ("mAne1?hAye", "mAne?hAye"),
    ("Sivei", "Siveyi"),
    ("?asfbAr", "?asafbAr"),
    ("Cetr", "Catr"),
    ("Cetre1", "Catre1"),
    ("tahtet", "tahte"),
    ("tahteye1", "tahte1"),
    ("tahteye", "tahte1"),
    ("bAznemud", "bAznamud"),
    ("bAznemude1", "bAznamude1"),
    ("bAznemudi", "bAznamudi"),
    ("bAznemudhA", "bAznamudhA"),
    ("bAznemudan", "bAznamudan"),
    ("nemudAr", "namudAr"),
    ("nemudArhA", "namudArhA"),
];
pub const WORD_FIX: &[(&str, &str)] = &[

];
pub const WORD_FIX_PAIRS: &[((&str, &str), (&str, &str))] = &[
    (("جو", "فریمن"), ("jo", "frimen")),
];
pub const NAMES: &[&str] = &["?ali", "?engeles", "?etiyen", "fredriS", "frimen", "kAlvan", "lAbo?esi", "mesnAr", "piSevari", "piyer", "ruhollAh", "safarxAn"];
pub const EZAFE_FORM: &[(&str, &str)] = &[
    ("no?", "nove1"),
];
pub const KASRA_END: &str = "ِ[ّْ]*[^\\w\u{600}-ۿ]*$";
pub const LATIN_WORD: &str = "[A-Za-zÀ-ÿ][A-Za-zÀ-ÿ'’]*";
pub const BUILTIN_LEXICON: &[(&str, &str)] = &[

];
pub const NUMBER_PH: &[&str] = &["?o", "CAhAr", "CAhArdah", "CAhArsad", "CAr", "CArdah", "Cehel", "SAnzdah", "Sast", "SeS", "SeSsad", "bist", "dah", "davAzdah", "devist", "do", "haSt", "haStAd", "haStsad", "haft", "haftAd", "haftsad", "hefdah", "hezAr", "hijdah", "milyArd", "milyun", "navad", "noh", "nohsad", "nuzdah", "pAnsad", "pAnzdah", "panj", "panjAh", "sad", "se", "si", "sisad", "sizdah", "vo", "yAzdah", "yek"];

pub const SENTENCE_SPLIT: &str = "(?<=[.!?؟])\\s+";
pub const PHRASE_SPLIT: &str = "(?<=[،؛:—–,;:)])\\s+|\\s+(?=\\()";
pub const STRONG_LEADIN: &[&str] = &[":", "؛", "—", "–"];
pub const TEXT_PREPS: &[&str] = &["بدون", "برای", "درباره", "مثل"];
pub const TEXT_LIGHT_VERBS: &[&str] = &["است", "باشد", "باشند", "بدهد", "بدهند", "بود", "بودم", "بودند", "بودی", "بودید", "بودیم", "داد", "دادم", "دادند", "داده", "دادی", "دادید", "دادیم", "دارد", "دارم", "دارند", "داری", "دارید", "داریم", "داشت", "شد", "شدم", "شدند", "شدی", "شدید", "شدیم", "شود", "شوند", "میباشد", "میباشند", "میداد", "میدادند", "میدهد", "میدهند", "میشود", "میشوند", "میکرد", "میکردند", "میکند", "میکنند", "هست", "هستم", "هستند", "هستی", "هستید", "هستیم", "کرد", "کردم", "کردند", "کرده", "کردی", "کردید", "کردیم", "کند", "کنم", "کنند", "کنی", "کنید", "کنیم"];
pub const TTS_PREPS: &[&str] = &["?az", "bA", "barAye", "be", "bedune", "dar", "ruye", "tA", "vase"];
pub const TTS_FUNCTION_WORDS: &[&str] = &["?az", "age", "bA", "barAye", "be", "bedune", "besurate", "beture", "dAr", "dar", "ke", "mi", "tA", "va", "vali", "vase", "yA"];
pub const TTS_CONJUNCTIONS: &[&str] = &["amA", "hattA", "ke", "rA", "va", "vali", "yA"];
pub const TTS_LIGHT_VERBS: &[&str] = &["?ast", "Ast", "Savad", "Savand", "Sod", "Sodan", "Sodand", "bud", "budand", "dAd", "dAdam", "dAdand", "dAde", "dAdi", "dAdid", "dAdim", "dAhad", "dAhand", "dArad", "dArand", "dAsht", "kard", "karde", "konad", "konand", "miSavad", "miSavam", "miSavand", "miSavid", "miSavim", "midAd", "midAdand", "mikonad", "mikonam", "mikonand", "mikonid", "mikonim"];

pub const G2P_LATIN_DIGRAPHS: &[(&str, &str)] = &[
    ("sh", "ش"),
    ("ch", "چ"),
    ("th", "ث"),
    ("ph", "ف"),
    ("gh", "غ"),
    ("kh", "خ"),
    ("wh", "و"),
    ("ck", "ک"),
    ("qu", "کو"),
    ("oo", "و"),
    ("ee", "ی"),
    ("ea", "ی"),
    ("ou", "او"),
    ("au", "او"),
    ("ai", "ای"),
    ("ay", "ای"),
    ("ey", "ای"),
    ("oi", "اوی"),
    ("oy", "اوی"),
];
pub const G2P_LATIN_CHAR: &[(&str, &str)] = &[
    ("a", "ا"),
    ("b", "ب"),
    ("c", "ک"),
    ("d", "د"),
    ("e", ""),
    ("f", "ف"),
    ("g", "گ"),
    ("h", "ه"),
    ("i", "ی"),
    ("j", "ج"),
    ("k", "ک"),
    ("l", "ل"),
    ("m", "م"),
    ("n", "ن"),
    ("o", "او"),
    ("p", "پ"),
    ("q", "ق"),
    ("r", "ر"),
    ("s", "س"),
    ("t", "ت"),
    ("u", "و"),
    ("v", "و"),
    ("w", "و"),
    ("x", "کس"),
    ("y", "ی"),
    ("z", "ز"),
];
pub const G2P_LETTER_NAMES: &[(&str, &str)] = &[
    ("a", "ای"),
    ("b", "بی"),
    ("c", "سی"),
    ("d", "دی"),
    ("e", "ای"),
    ("f", "اف"),
    ("g", "جی"),
    ("h", "اچ"),
    ("i", "آی"),
    ("j", "جی"),
    ("k", "کی"),
    ("l", "ال"),
    ("m", "ام"),
    ("n", "ان"),
    ("o", "او"),
    ("p", "پی"),
    ("q", "کیو"),
    ("r", "آر"),
    ("s", "اس"),
    ("t", "تی"),
    ("u", "یو"),
    ("v", "وی"),
    ("w", "دبلیو"),
    ("x", "ایکس"),
    ("y", "وای"),
    ("z", "زد"),
];
pub const G2P_LATIN_EXCEPTIONS: &[(&str, &str)] = &[
    ("echo", "اکو"),
    ("state", "استیت"),
    ("network", "نت\u{200c}ورک"),
    ("windows", "ویندوز"),
    ("notebook", "نوت\u{200c}بوک"),
    ("photoshop", "فتوشاپ"),
    ("python", "پایتون"),
    ("recurrency", "ریکارانسی"),
];
pub const G2P_LATIN_WORD: &str = "[A-Za-z][A-Za-z'-]*";
pub const TO_PHONEMES: &[(char, &str)] = &[('/', "a"), ('a', "A"), ('@', "?"), ('$', "S"), ('c', "C")];
pub const G2P_PRON_FIX: &[(&str, &str)] = &[
    ("bo?d", "ba?d"),
];
pub const G2P_G2P_CUT_WORDS: &[&str] = &["اما", "بنابراین", "زیرا", "سپس", "همچنین", "و", "ولی", "پس", "چون", "که", "یا"];
pub const CLUSTER_START: &str = "بتثجذزسشصضفپچژکگ";

pub const NF_ZWNJ: &str = "\u{200c}";
pub const NF_PERSIAN_LETTERS: &str = "آابپتثجچحخدذرزژسشصضطظعغفقکگلمنوهیئ";
pub const NF_KEPT_PUNCT: &str = ".،؛؟!:";
pub const NF_CHAR_MAP: &[(&str, &str)] = &[
    ("ي", "ی"),
    ("ى", "ی"),
    ("ۍ", "ی"),
    ("ې", "ی"),
    ("ك", "ک"),
    ("ڪ", "ک"),
    ("ة", "ه"),
    ("ۀ", "ه"),
    ("أ", "ا"),
    ("إ", "ا"),
    ("ٱ", "ا"),
    ("ٲ", "ا"),
    ("ٳ", "ا"),
    ("ؤ", "و"),
    ("ۂ", "ه"),
    ("ۃ", "ه"),
    ("ء", ""),
    ("ـ", ""),
];
pub const NF_DIACRITICS: &str = "[ً-ٕ\u{670}\u{6d6}-\u{6ed}ٖ-ٟ]";
pub const NF_INVISIBLES: &str = "[\u{200b}\u{200d}\u{200e}\u{200f}\u{202a}-\u{202e}\u{2066}-\u{2069}\u{feff}\u{ad}]";
pub const NF_PUNCT_MAP: &[(&str, &str)] = &[
    (",", "،"),
    ("?", "؟"),
    (";", "؛"),
    ("٬", ""),
    ("٫", "."),
    ("…", "."),
    ("»", " "),
    ("«", " "),
    ("“", " "),
    ("”", " "),
    ("‘", " "),
    ("’", " "),
    ("\"", " "),
    ("'", " "),
    ("`", " "),
    ("(", " "),
    (")", " "),
    ("[", " "),
    ("]", " "),
    ("{", " "),
    ("}", " "),
    ("–", " "),
    ("—", " "),
    ("−", " "),
    ("-", " "),
    ("‐", " "),
    ("/", " "),
    ("\\", " "),
    ("*", " "),
    ("_", " "),
    ("|", " "),
    ("×", " "),
    ("=", " "),
    ("+", " "),
    ("،", "،"),
    ("؛", "؛"),
    ("؟", "؟"),
];
pub const NF_ALLOWED: &str = " !.:،؛؟آئابتثجحخدذرزسشصضطظعغفقلمنهوپچژکگی\u{200c}";
pub const NF_ONES: &[&str] = &["", "یک", "دو", "سه", "چهار", "پنج", "شش", "هفت", "هشت", "نه"];
pub const NF_TEENS: &[&str] = &["ده", "یازده", "دوازده", "سیزده", "چهارده", "پانزده", "شانزده", "هفده", "هجده", "نوزده"];
pub const NF_TENS: &[&str] = &["", "", "بیست", "سی", "چهل", "پنجاه", "شصت", "هفتاد", "هشتاد", "نود"];
pub const NF_HUNDREDS: &[&str] = &["", "صد", "دویست", "سیصد", "چهارصد", "پانصد", "ششصد", "هفتصد", "هشتصد", "نهصد"];
pub const CP1256_HIGH: &str = "€پ‚ƒ„…†‡ˆ‰ٹ‹Œچژڈگ‘’“”•–—ک™ڑ›œ\u{200c}\u{200d}ں\u{a0}،¢£¤¥¦§¨©ھ«¬\u{ad}®¯°±²³´µ¶·¸¹؛»¼½¾؟ہءآأؤإئابةتثجحخدذرزسشصض×طظعغـفقكàلâمنهوçèéêëىيîïًٌٍَôُِ÷ّùْûü\u{200e}\u{200f}ے";

pub const JOINS_NEXT: &str = "ﭔﭕﭘﭙﭜﭝﭠﭡﭤﭥﭨﭩﭬﭭﭰﭱﭴﭵﭸﭹﭼﭽﮀﮁﮐﮑﮔﮕﮘﮙﮜﮝﮢﮣﮨﮩﮬﮭﯕﯖﯦﯧﯨﯩﯸﯻﯾﯿﲗﲘﲙﲚﲛﲜﲝﲞﲟﲠﲡﲢﲣﲤﲥﲦﲧﲨﲩﲪﲫﲬﲭﲮﲯﲰﲱﲲﲳﲴﲵﲶﲷﲸﲹﲺﲻﲼﲽﲾﲿﳀﳁﳂﳃﳄﳅﳆﳇﳈﳉﳊﳋﳌﳍﳎﳏﳐﳑﳒﳓﳔﳕﳖﳗﳘﳙﳚﳛﳜﳝﳞﳟﳠﳡﳢﳣﳤﳥﳦﳧﳨﳩﳪﳫﳬﳭﳮﳯﳰﳱﳲﳳﳴﴭﴮﴯﴰﴱﴲﴳﴴﴵﴶﴷﴸﴹﴺﴻﵐﵒﵓﵔﵕﵖﵗﵙﵜﵝﵠﵡﵣﵥﵨﵫﵭﵰﵲﵳﵷﵽﶃﶆﶈﶉﶊﶌﶍﶎﶏﶒﶓﶔﶕﶘﶝﶴﶵﶸﶺﷃﷄﷅﹷﹹﹻﹽﹿﺋﺌﺑﺒﺗﺘﺛﺜﺟﺠﺣﺤﺧﺨﺳﺴﺷﺸﺻﺼﺿﻀﻃﻄﻇﻈﻋﻌﻏﻐﻓﻔﻗﻘﻛﻜﻟﻠﻣﻤﻧﻨﻫﻬﻳﻴ";
pub const JOINS_PREV: &str = "ﭑﭓﭕﭗﭙﭛﭝﭟﭡﭣﭥﭧﭩﭫﭭﭯﭱﭳﭵﭷﭹﭻﭽﭿﮁﮃﮅﮇﮉﮋﮍﮏﮑﮓﮕﮗﮙﮛﮝﮟﮡﮣﮥﮧﮩﮫﮭﮯﮱﯔﯖﯘﯚﯜﯟﯡﯣﯥﯧﯩﯫﯭﯯﯱﯳﯵﯷﯺﯽﯿﱤﱥﱦﱧﱨﱩﱪﱫﱬﱭﱮﱯﱰﱱﱲﱳﱴﱵﱶﱷﱸﱹﱺﱻﱼﱽﱾﱿﲀﲁﲂﲃﲄﲅﲆﲇﲈﲉﲊﲋﲌﲍﲎﲏﲐﲑﲒﲓﲔﲕﲖﳟﳠﳡﳢﳣﳤﳥﳦﳧﳨﳩﳪﳫﳬﳭﳮﳯﳰﳱﳲﳳﳴﴑﴒﴓﴔﴕﴖﴗﴘﴙﴚﴛﴜﴝﴞﴟﴠﴡﴢﴣﴤﴥﴦﴧﴨﴩﴪﴫﴬﴴﴵﴶﴷﴸﴹﴺﴻﴼﵑﵘﵚﵛﵞﵟﵢﵤﵦﵧﵩﵪﵬﵮﵯﵱﵴﵵﵶﵸﵹﵺﵻﵼﵾﵿﶀﶁﶂﶄﶅﶇﶋﶖﶗﶙﶚﶛﶜﶞﶟﶠﶡﶢﶣﶤﶥﶦﶧﶨﶩﶪﶫﶬﶭﶮﶯﶰﶱﶲﶳﶶﶷﶹﶻﶼﶽﶾﶿﷀﷁﷂﷆﷇﹷﹹﹻﹽﹿﺂﺄﺆﺈﺊﺌﺎﺐﺒﺔﺖﺘﺚﺜﺞﺠﺢﺤﺦﺨﺪﺬﺮﺰﺲﺴﺶﺸﺺﺼﺾﻀﻂﻄﻆﻈﻊﻌﻎﻐﻒﻔﻖﻘﻚﻜﻞﻠﻢﻤﻦﻨﻪﻬﻮﻰﻲﻴﻶﻸﻺﻼ";

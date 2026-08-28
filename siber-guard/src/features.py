"""Domain adindan leksik/istatistiksel ozellik cikarimi.

Yontem, DGA (Domain Generation Algorithm) ve oltalama (phishing) domain
tespiti uzerine akademik literaturde standart olan yaklasimdir: domain
icerigini degil, *sekil ozelliklerini* (uzunluk, entropi, rakam orani,
supheli anahtar kelimeler, marka taklidi vb.) kullanir. Boylece HTTPS
trafigini deşifre etmeye gerek kalmadan, sadece sorgulanan domain adindan
karar verilebilir.

v2 notu: Bu surumde "eTLD+1" (asil marka/kayitli domain) hesabi, cok
parcali uzantilari (.com.tr, .gov.tr, .co.uk vb.) tanıyan kucuk, gomulu bir
"public suffix" alt kumesiyle yapiliyor. Onceki surum sadece son etiketi
(ör. "tr") TLD sayiyor, ondan onceki etiketi (ör. "gov") marka sanıyordu --
Turkce'de "e-devlet.gov.tr" gibi son derece yaygin adresler icin bu yanlisti.
"""
from __future__ import annotations

import math
import re
from collections import Counter

SUSPICIOUS_KEYWORDS = [
    "login", "verify", "secure", "account", "update", "confirm", "signin",
    "banking", "password", "wallet", "support", "billing", "invoice",
    "giris", "dogrula", "guvenlik", "hesap", "sifre", "odeme", "destek",
    "kazandiniz", "hediye", "ucretsiz", "acildi", "onay", "iade", "kargo",
]

SUSPICIOUS_TLDS = {
    "top", "xyz", "club", "work", "support", "click", "link", "zip",
    "gq", "tk", "ml", "ga", "cf", "buzz", "rest", "quest", "loan", "win",
    "surf", "cam", "cyou", "monster", "icu", "bar", "party",
}

KNOWN_BRANDS = [
    "google", "facebook", "instagram", "whatsapp", "apple", "microsoft",
    "paypal", "amazon", "netflix", "trendyol", "garanti", "isbank",
    "akbank", "ziraat", "yapikredi", "turkiye", "gov", "e-devlet",
    "hepsiburada", "n11", "getir", "turkishairlines", "denizbank",
    "vakifbank", "halkbank", "sahibinden", "migros",
]

# Kucuk, gomulu bir "public suffix" alt kumesi -- tam Public Suffix List
# (publicsuffix.org) binlerce kural icerir ve internet erisimi gerektirir.
# Bu ortamda canli indirme mumkun olmadigi icin, Turkce ve en yaygin
# uluslararasi cok-parcali uzantilar elle derlendi. Kendi sunucunda tam
# internet erisimin varsa, `tldextract` kutuphanesiyle degistirebilirsin
# (bkz. README "Gelistirme fikirleri").
MULTI_PART_SUFFIXES = {
    # Turkiye
    "com.tr", "gov.tr", "edu.tr", "org.tr", "net.tr", "biz.tr", "info.tr",
    "web.tr", "gen.tr", "av.tr", "bel.tr", "pol.tr", "mil.tr", "k12.tr",
    "tv.tr", "name.tr", "tel.tr", "dr.tr",
    # Birlesik Krallik
    "co.uk", "org.uk", "ac.uk", "gov.uk", "me.uk", "ltd.uk", "plc.uk",
    # Diger yaygin ulke kodlari
    "com.au", "net.au", "org.au", "co.jp", "ne.jp", "co.kr", "co.nz",
    "com.br", "co.in", "co.za", "com.mx", "com.ar", "com.sg", "com.hk",
    "co.il", "com.tw", "com.cn", "co.id",
}

# Bu uzantilarin kaydi, ilgili devlet/egitim kurumlarinca dogrulanmis
# kimlik/kurum belgesi gerektirir -- bir saldirganin ".gov.tr" veya
# ".edu.tr" altinda alan adi almasi pratikte cok zordur. Bu yuzden bu
# uzantilar guclu bir "resmi/guvenilir" sinyalidir ve tire/uzunluk gibi
# yanitici olmayan ozelliklerin yanlis-pozitif uretmesini dengeler.
REGULATED_SUFFIXES = {
    "gov.tr", "edu.tr", "mil.tr", "pol.tr", "k12.tr",
    "gov.uk", "ac.uk", "gov.au", "gov",
}

_VOWELS = set("aeiouAEIOU")
_PUNYCODE_PREFIX = "xn--"


def shannon_entropy(s: str) -> float:
    if not s:
        return 0.0
    counts = Counter(s)
    length = len(s)
    return -sum((c / length) * math.log2(c / length) for c in counts.values())


def _levenshtein(a: str, b: str) -> int:
    if a == b:
        return 0
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i] + [0] * len(b)
        for j, cb in enumerate(b, 1):
            cur[j] = min(
                prev[j] + 1,
                cur[j - 1] + 1,
                prev[j - 1] + (ca != cb),
            )
        prev = cur
    return prev[-1]


def min_brand_distance(label: str) -> int:
    if not KNOWN_BRANDS:
        return 99
    return min(_levenshtein(label, b) for b in KNOWN_BRANDS)


def contains_brand_substring(label: str) -> str | None:
    """Combosquatting tespiti: 'trendyolshop' gibi, marka adi tam degil
    ama icinde geciyor (typo mesafesi 0 degil, substring eslesmesi var)."""
    for brand in KNOWN_BRANDS:
        if len(brand) >= 4 and brand in label and label != brand:
            return brand
    return None


def split_registrable_and_suffix(domain: str) -> tuple[str, str, int]:
    """Domain'i (kayitli/marka kismi, uzanti, alt-domain sayisi) uclusune ayirir.

    Cok parcali uzantilari (MULTI_PART_SUFFIXES) tanir; ör.
    'giris.e-devlet.gov.tr' -> ('e-devlet', 'gov.tr', 1)
    'trendyol.com'          -> ('trendyol', 'com', 0)
    """
    labels = domain.split(".")
    if len(labels) < 2:
        return domain, "", 0

    last_two = ".".join(labels[-2:])
    if last_two in MULTI_PART_SUFFIXES and len(labels) >= 3:
        registrable = labels[-3]
        suffix = last_two
        subdomain_count = max(len(labels) - 3, 0)
    else:
        registrable = labels[-2]
        suffix = labels[-1]
        subdomain_count = max(len(labels) - 2, 0)
    return registrable, suffix, subdomain_count


def extract_features(domain: str) -> dict:
    domain = domain.strip().lower().rstrip(".")
    # Kullanicilar bazen "https://ornek.com:8443" gibi port/scheme icerebilir
    has_port = ":" in domain and not re.search(r"^(\d{1,3}\.){3}\d{1,3}$", domain.split(":")[0])
    domain_no_port = domain.split(":")[0]

    registrable, tld, subdomain_count = split_registrable_and_suffix(domain_no_port)

    letters = [c for c in registrable if c.isalpha()]
    digits = [c for c in registrable if c.isdigit()]
    hyphens = registrable.count("-")
    vowels = [c for c in letters if c in _VOWELS]

    is_ip_literal = bool(re.fullmatch(r"(\d{1,3}\.){3}\d{1,3}", domain_no_port))
    has_suspicious_keyword = any(k in domain_no_port for k in SUSPICIOUS_KEYWORDS)
    has_suspicious_tld = tld in SUSPICIOUS_TLDS or tld.split(".")[-1] in SUSPICIOUS_TLDS
    brand_dist = min_brand_distance(registrable)
    looks_like_brand_typo = 0 < brand_dist <= 2
    combosquat_brand = contains_brand_substring(registrable)
    is_combosquat = combosquat_brand is not None and brand_dist != 0
    is_punycode = any(label.startswith(_PUNYCODE_PREFIX) for label in domain_no_port.split("."))
    is_regulated_suffix = tld in REGULATED_SUFFIXES

    return {
        "length": len(domain_no_port),
        "registrable_length": len(registrable),
        "subdomain_count": subdomain_count,
        "digit_ratio": (len(digits) / len(registrable)) if registrable else 0.0,
        "hyphen_count": hyphens,
        "vowel_ratio": (len(vowels) / len(letters)) if letters else 0.0,
        "entropy": shannon_entropy(registrable),
        "is_ip_literal": int(is_ip_literal),
        "has_suspicious_keyword": int(has_suspicious_keyword),
        "has_suspicious_tld": int(has_suspicious_tld),
        "brand_edit_distance": brand_dist,
        "looks_like_brand_typo": int(looks_like_brand_typo),
        "is_combosquat": int(is_combosquat),
        "is_punycode": int(is_punycode),
        "is_regulated_suffix": int(is_regulated_suffix),
        "has_port": int(has_port),
        "tld_length": len(tld),
        "dot_count": domain_no_port.count("."),
        "max_consecutive_consonants": _max_consecutive_consonants(registrable),
    }


def _max_consecutive_consonants(s: str) -> int:
    best = cur = 0
    for c in s:
        if c.isalpha() and c not in _VOWELS:
            cur += 1
            best = max(best, cur)
        else:
            cur = 0
    return best


FEATURE_ORDER = [
    "length", "registrable_length", "subdomain_count", "digit_ratio",
    "hyphen_count", "vowel_ratio", "entropy", "is_ip_literal",
    "has_suspicious_keyword", "has_suspicious_tld", "brand_edit_distance",
    "looks_like_brand_typo", "is_combosquat", "is_punycode",
    "is_regulated_suffix", "has_port", "tld_length", "dot_count",
    "max_consecutive_consonants",
]


def feature_vector(domain: str) -> list:
    feats = extract_features(domain)
    return [feats[k] for k in FEATURE_ORDER]

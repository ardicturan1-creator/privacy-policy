"""Domain adindan leksik/istatistiksel ozellik cikarimi.

Yontem, DGA (Domain Generation Algorithm) ve oltalama (phishing) domain
tespiti uzerine akademik literaturde standart olan yaklasimdir: domain
icerigini degil, *sekil ozelliklerini* (uzunluk, entropi, rakam orani,
supheli anahtar kelimeler, marka taklidi vb.) kullanir. Boylece HTTPS
trafigini deşifre etmeye gerek kalmadan, sadece sorgulanan domain adindan
karar verilebilir.
"""
from __future__ import annotations

import math
import re
from collections import Counter

SUSPICIOUS_KEYWORDS = [
    "login", "verify", "secure", "account", "update", "confirm", "signin",
    "banking", "password", "wallet", "support", "billing", "invoice",
    "giris", "dogrula", "guvenlik", "hesap", "sifre", "odeme", "destek",
    "kazandiniz", "hediye", "ucretsiz", "acildi", "onay",
]

SUSPICIOUS_TLDS = {
    "top", "xyz", "club", "work", "support", "click", "link", "zip",
    "gq", "tk", "ml", "ga", "cf", "buzz", "rest", "quest", "loan", "win",
}

KNOWN_BRANDS = [
    "google", "facebook", "instagram", "whatsapp", "apple", "microsoft",
    "paypal", "amazon", "netflix", "trendyol", "garanti", "isbank",
    "akbank", "ziraat", "yapikredi", "turkiye", "gov", "e-devlet",
]

_VOWELS = set("aeiouAEIOU")


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


def extract_features(domain: str) -> dict:
    domain = domain.strip().lower().rstrip(".")
    labels = domain.split(".")
    tld = labels[-1] if len(labels) > 1 else ""
    registrable = labels[-2] if len(labels) > 1 else domain
    subdomain_count = max(len(labels) - 2, 0)

    letters = [c for c in registrable if c.isalpha()]
    digits = [c for c in registrable if c.isdigit()]
    hyphens = registrable.count("-")
    vowels = [c for c in letters if c in _VOWELS]

    is_ip_literal = bool(re.fullmatch(r"(\d{1,3}\.){3}\d{1,3}", domain))
    has_suspicious_keyword = any(k in domain for k in SUSPICIOUS_KEYWORDS)
    has_suspicious_tld = tld in SUSPICIOUS_TLDS
    brand_dist = min_brand_distance(registrable)
    looks_like_brand_typo = 0 < brand_dist <= 2

    return {
        "length": len(domain),
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
        "tld_length": len(tld),
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
    "looks_like_brand_typo", "tld_length", "max_consecutive_consonants",
]


def feature_vector(domain: str) -> list:
    feats = extract_features(domain)
    return [feats[k] for k in FEATURE_ORDER]

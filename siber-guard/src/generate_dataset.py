"""Etiketli egitim veri seti olusturur: gercek guvenilir domainler +
belgelenmis phishing/DGA desenlerinden turetilmis sentetik zararli domainler.

NOT: Bu ortamin ag politikasi canli tehdit-istihbarati feed'lerine
(URLhaus, OpenPhish, Tranco) erisimi engelliyor. Bu yuzden zararli ornekler,
guvenlik literaturunde belgelenmis iki teknikle *algoritmik olarak* uretilir:

  1. Marka taklidi (typosquatting): bilinen bir markanin adina karakter
     ekleme/cikarma/degistirme + supheli TLD veya "login/verify/secure" gibi
     kelimeler eklenmesi. (Bkz. gercek oltalama kampanyalarinda yaygin desen.)
  2. DGA-tarzi rastgele string: yuksek entropili, unsuz-agirlikli rastgele
     karakter dizileri + kotu amacli yazilimlarin C2 altyapisinda sikca
     gorulen ucuz/anonim TLD'ler (.top, .xyz, .tk, .club vb.)

Gercek dagitimda bu betik, data/benign_domains.txt yerine canli bir Tranco/
Majestic listesiyle ve data/malicious_domains.txt yerine URLhaus/OpenPhish/
PhishTank canli feed'leriyle degistirilmelidir (bkz. README "Gercek veriyle
degistirme" bolumu).
"""
from __future__ import annotations

import csv
import pathlib
import random

random.seed(42)

HERE = pathlib.Path(__file__).resolve().parent
DATA_DIR = HERE.parent / "data"

BRANDS = [
    "google", "facebook", "instagram", "whatsapp", "apple", "microsoft",
    "paypal", "amazon", "netflix", "trendyol", "garanti", "isbank",
    "akbank", "ziraat", "yapikredi", "e-devlet", "turkiye", "gov",
]

SUSPICIOUS_PREFIXES = [
    "login", "secure", "verify", "update", "account", "confirm", "support",
    "giris", "guvenlik", "dogrula", "hesap", "odeme", "destek", "kazandin",
]

SUSPICIOUS_TLDS = [
    "top", "xyz", "club", "work", "support", "click", "link", "zip",
    "gq", "tk", "ml", "ga", "cf", "buzz", "rest", "quest", "loan", "win",
]

CONSONANTS = "bcdfghjklmnpqrstvwxyz"
VOWELS = "aeiou"


def typosquat_variants(brand: str, n: int) -> list[str]:
    out = set()
    ops = ["insert", "delete", "substitute", "hyphen", "prefix_suffix"]
    attempts = 0
    while len(out) < n and attempts < n * 20:
        attempts += 1
        op = random.choice(ops)
        b = list(brand)
        if op == "insert" and b:
            i = random.randrange(len(b) + 1)
            b.insert(i, random.choice(CONSONANTS + VOWELS))
        elif op == "delete" and len(b) > 3:
            i = random.randrange(len(b))
            del b[i]
        elif op == "substitute" and b:
            i = random.randrange(len(b))
            b[i] = random.choice(CONSONANTS + VOWELS)
        elif op == "hyphen" and len(b) > 2:
            i = random.randrange(1, len(b))
            b.insert(i, "-")
        registrable = "".join(b)
        if op == "prefix_suffix" or random.random() < 0.5:
            piece = random.choice(SUSPICIOUS_PREFIXES)
            registrable = f"{piece}-{registrable}" if random.random() < 0.5 else f"{registrable}-{piece}"
        tld = random.choice(SUSPICIOUS_TLDS + ["com", "net", "info"])
        domain = f"{registrable}.{tld}"
        if domain != f"{brand}.com":
            out.add(domain)
    return list(out)[:n]


def dga_variants(n: int) -> list[str]:
    out = set()
    while len(out) < n:
        length = random.randint(10, 22)
        chars = []
        use_consonant = random.random() < 0.5
        for _ in range(length):
            if use_consonant:
                chars.append(random.choice(CONSONANTS))
            else:
                chars.append(random.choice(CONSONANTS + VOWELS))
            use_consonant = random.random() < 0.75
        registrable = "".join(chars)
        tld = random.choice(SUSPICIOUS_TLDS)
        subdomain_noise = ""
        if random.random() < 0.2:
            subdomain_noise = "".join(random.choice(CONSONANTS + VOWELS) for _ in range(6)) + "."
        out.add(f"{subdomain_noise}{registrable}.{tld}")
    return list(out)[:n]


def ip_literal_variants(n: int) -> list[str]:
    out = set()
    while len(out) < n:
        octets = [str(random.randint(1, 254)) for _ in range(4)]
        out.add(".".join(octets))
    return list(out)[:n]


def build_dataset() -> None:
    benign_path = DATA_DIR / "benign_domains.txt"
    benign = [
        line.strip().lower()
        for line in benign_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    benign = sorted(set(benign))

    per_brand = max(1, (len(benign) * 2) // (len(BRANDS) * 3))
    malicious: list[str] = []
    for brand in BRANDS:
        malicious.extend(typosquat_variants(brand, per_brand))
    malicious.extend(dga_variants(len(benign)))
    malicious.extend(ip_literal_variants(max(20, len(benign) // 6)))
    malicious = sorted(set(malicious) - set(benign))

    rows = [(d, 0) for d in benign] + [(d, 1) for d in malicious]
    random.shuffle(rows)

    out_path = DATA_DIR / "dataset.csv"
    with out_path.open("w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["domain", "label"])
        w.writerows(rows)

    print(f"guvenilir (0): {len(benign)}")
    print(f"zararli/supheli (1): {len(malicious)}")
    print(f"toplam: {len(rows)} -> {out_path}")


if __name__ == "__main__":
    build_dataset()

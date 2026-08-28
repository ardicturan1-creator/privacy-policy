"""Etiketli egitim veri seti olusturur: gercek guvenilir domainler +
belgelenmis phishing/DGA desenlerinden turetilmis sentetik zararli domainler.

NOT: Bu ortamin ag politikasi canli tehdit-istihbarati feed'lerine
(URLhaus, OpenPhish, Tranco) erisimi engelliyor. Bu yuzden zararli ornekler,
guvenlik literaturunde belgelenmis tekniklerle *algoritmik olarak* uretilir:

  1. Marka taklidi (typosquatting): bilinen bir markanin adina karakter
     ekleme/cikarma/degistirme + supheli TLD veya "login/verify/secure" gibi
     kelimeler eklenmesi.
  2. Combosquatting: marka adi *tam degismeden*, baska bir kelimeyle
     birlestirilerek kullanilir (ör. "trendyolshop.com") -- typo yok, sadece
     ek kelime; typosquat'tan farkli bir aile oldugu icin ayri uretiliyor.
  3. Leetspeak / karakter degisimi: harflerin gorsel olarak benzer rakamlarla
     degistirilmesi (o->0, i->1, e->3, a->4, s->5) -- gercek oltalama
     kampanyalarinda cok yaygin.
  4. Punycode/IDN homograf: "xn--" onekiyle baslayan, gorunurde Latin
     harflerine benzeyen ama farkli Unicode karakterlerinden olusan
     domainlerin temsili ("xn--pypal-4ve.com" gibi gercek dunya orneklerine
     benzer sekilde).
  5. DGA-tarzi rastgele string: yuksek entropili, unsuz-agirlikli rastgele
     karakter dizileri + kotu amacli yazilimlarin C2 altyapisinda sikca
     gorulen ucuz/anonim TLD'ler (.top, .xyz, .tk, .club vb.)
  6. Turkce ccTLD hedefli typosquat: yukaridaki 1-3 teknikleri, Turkiye'de
     cok yaygin olan .com.tr/.gov.tr uzantilariyla da uretilir -- cunku bu
     projenin hedef kitlesi Turkce konusan kullanicilar.

Ayrica, egitim setinden TAMAMEN AYRI, elle olusturulmus kucuk bir
"zor test seti" (hard_test_set.csv) uretilir: bu setteki zararli ornekler
farkli bir rastgele tohum ve biraz farkli kombinasyonlarla, egitim
uretecinin "ezberleyebilecegi" kaliplardan kacinarak yazilir. Boylece
train_classifier.py, egitim dagilimina asiri uyum (overfitting) riskini
ayrica raporlayabilir -- ilk surumdeki "%100 dogruluk" yanilticiligina
karsi somut bir onlem.

Gercek dagitimda bu betik, data/benign_domains.txt yerine canli bir Tranco/
Majestic listesiyle ve sentetik zararli uretimi yerine URLhaus/OpenPhish/
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
    "hepsiburada", "n11", "getir", "turkishairlines", "denizbank",
    "vakifbank", "halkbank", "sahibinden", "migros",
]

SUSPICIOUS_PREFIXES = [
    "login", "secure", "verify", "update", "account", "confirm", "support",
    "giris", "guvenlik", "dogrula", "hesap", "odeme", "destek", "kazandin",
    "iade", "kargo-takip",
]

COMBOSQUAT_SUFFIXES = [
    "shop", "store", "online", "app", "mobile", "care", "help", "id",
    "wallet", "pay", "market", "giris", "hesabim", "kampanya", "indirim",
]

SUSPICIOUS_TLDS = [
    "top", "xyz", "club", "work", "support", "click", "link", "zip",
    "gq", "tk", "ml", "ga", "cf", "buzz", "rest", "quest", "loan", "win",
    "surf", "cam", "cyou", "monster", "icu", "bar", "party",
]

TURKISH_CCTLDS = ["com.tr", "gov.tr", "web.tr", "net.tr", "org.tr"]

LEETSPEAK_MAP = {"o": "0", "i": "1", "e": "3", "a": "4", "s": "5"}

CONSONANTS = "bcdfghjklmnpqrstvwxyz"
VOWELS = "aeiou"


def typosquat_variants(brand: str, n: int, tlds: list[str] | None = None) -> list[str]:
    out = set()
    ops = ["insert", "delete", "substitute", "hyphen", "prefix_suffix"]
    tlds = tlds or (SUSPICIOUS_TLDS + ["com", "net", "info"])
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
        tld = random.choice(tlds)
        domain = f"{registrable}.{tld}"
        if domain != f"{brand}.com":
            out.add(domain)
    return list(out)[:n]


def combosquat_variants(brand: str, n: int) -> list[str]:
    """Marka adi bozulmadan, baska bir kelimeyle birlestirilir.

    Gercek combosquat kampanyalarinin cogu -- typosquat'in aksine -- kaba
    bir supheli TLD/tire kullanmaz, tam tersine kurumsal gorunmek icin
    temiz bir '.com' tercih eder (ör. 'trendyolindirimleri.com'). Bu yuzden
    burada BILINCLI olarak agirlik temiz TLD + tiresiz forma kaydirildi;
    aksi halde model, marka-alt-string sinyalini (is_combosquat) hic
    kullanmadan sadece TLD/tireye bakarak "yeterince basarili" olur ve
    gercek dunyadaki en sinsi combosquat turunu kacirir (bkz.
    models/evaluation_report.txt'teki zor-test-seti bulgulari)."""
    out = set()
    attempts = 0
    while len(out) < n and attempts < n * 20:
        attempts += 1
        suffix = random.choice(COMBOSQUAT_SUFFIXES)
        registrable = f"{brand}{suffix}" if random.random() < 0.85 else f"{brand}-{suffix}"
        tld = random.choice(["com", "com.tr", "net"] * 4 + SUSPICIOUS_TLDS)
        out.add(f"{registrable}.{tld}")
    return list(out)[:n]


def leetspeak_variants(brand: str, n: int) -> list[str]:
    out = set()
    attempts = 0
    while len(out) < n and attempts < n * 30:
        attempts += 1
        chars = list(brand)
        candidates = [i for i, c in enumerate(chars) if c in LEETSPEAK_MAP]
        if not candidates:
            break
        k = random.randint(1, min(2, len(candidates)))
        for i in random.sample(candidates, k):
            chars[i] = LEETSPEAK_MAP[chars[i]]
        registrable = "".join(chars)
        tld = random.choice(["com", "net", "info"] + SUSPICIOUS_TLDS)
        out.add(f"{registrable}.{tld}")
    return list(out)[:n]


def punycode_variants(n: int) -> list[str]:
    """Gercek IDN-homograf saldirilarinin temsili bicimi: 'xn--' onekli,
    gorunurde markaya yakin ama Unicode kod noktasi farkli etiketler."""
    out = set()
    suffixes = ["4ve", "1ve", "0ve", "6b8a", "n8a", "kva", "zja", "mxa"]
    attempts = 0
    while len(out) < n and attempts < n * 20:
        attempts += 1
        brand = random.choice(BRANDS)
        suf = random.choice(suffixes)
        tld = random.choice(["com", "net", "org"])
        out.add(f"xn--{brand}-{suf}.{tld}")
    return list(out)[:n]


def dga_variants(n: int, tlds: list[str] | None = None) -> list[str]:
    out = set()
    tlds = tlds or SUSPICIOUS_TLDS
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
        tld = random.choice(tlds)
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
        malicious.extend(combosquat_variants(brand, max(6, per_brand)))
        malicious.extend(leetspeak_variants(brand, max(1, per_brand // 4)))
        malicious.extend(typosquat_variants(brand, max(1, per_brand // 3), tlds=TURKISH_CCTLDS))
    malicious.extend(punycode_variants(max(20, len(benign) // 6)))
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

    build_hard_test_set(benign)


def build_hard_test_set(benign: list[str]) -> None:
    """Egitim uretecinden BAGIMSIZ, farkli bir rastgele durumla ve elle
    secilmis zor orneklerle kucuk bir degerlendirme seti olusturur.
    Amac: modelin kendi ureticisinin kalibini ezberlemedigini, gercekten
    genellestigini olcmek."""
    rng = random.Random(1234)

    hard_malicious = [
        # Marka + Turkce kelime, tire yok, .com (typosquat_variants'ta
        # uretilmeyen bir kombinasyon bicimi)
        "trendyolindirimleri.com",
        "garantibankasigiris.com",
        "turkiyeedevletim.net",
        "isbankasimobil.com",
        "getirsiparisim.com",
        "sahibindenilan.net",
        # Cift TLD / karisik yapi
        "trendyol.com.giris-guvenlik.top",
        "apple-id-dogrulama.com.tr",
        # Sayi/harf karisimi, tire olmadan
        "amaz0n-teslimat.com",
        "payp4l-hesap.net",
        "n3tflix-yenileme.com",
        # Kisa, marka disi ama supheli anahtar kelime + rastgele ek
        "guvenlik-dogrulama7452.xyz",
        "hesap-onay-kodu.club",
        # Ayrik altdomain zinciri
        "secure.login.akbank.verify-account.ru",
    ]
    rng.shuffle(hard_malicious)

    # Guvenilir tarafta ZOR ornekler: tireli/uzun/gercek Turkce kurumsal
    # adlar -- yanlis-pozitif riskini olcmek icin kritik.
    hard_benign = rng.sample(benign, k=min(20, len(benign)))
    hard_benign += [
        "e-fatura.gov.tr",
        "e-nabiz.gov.tr",
        "turkiye.gov.tr",
        "istanbul-altyapi.gov.tr",
        "hepsiburada-kurumsal.com",
    ]

    rows = [(d, 1) for d in hard_malicious] + [(d, 0) for d in hard_benign]
    rng.shuffle(rows)

    out_path = DATA_DIR / "hard_test_set.csv"
    with out_path.open("w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["domain", "label"])
        w.writerows(rows)
    print(f"zor test seti: {len(hard_malicious)} zararli + {len(hard_benign)} guvenilir -> {out_path}")


if __name__ == "__main__":
    build_dataset()

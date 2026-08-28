#!/usr/bin/env python3
"""'Bu program virus mu?' sorusu icin dosya itibar kontrolu.

ONEMLI - durustluk notu:
Bir calistirilabilir dosyanin zararli olup olmadigini, dosyanin kendisini
gormeden (yani sifirdan, kucuk bir modelle, egitim verisi olmadan) guvenilir
sekilde soylemek MUMKUN DEGILDIR. Ciddi bir ikili-dosya siniflandirici, on
binlerce etiketli malware/temiz-dosya ornegi, PE/ELF header ayrıstirma,
genellikle bir sandbox'ta calistirip davranis izleme gerektirir -- bu
projenin "sıfırdan küçük sınıflandırıcı" kapsamini fazlasiyla asar ve
guvenilmez/yanlis-guven-verici bir sonuc uretme riski tasir.

Bu yuzden burada UYDURMA bir "virus skoru" hesaplamiyoruz. Onun yerine
ENDÜSTRI STANDARDI, sorumlu yontemi kullaniyoruz: dosyanin SHA-256 ozetini
hesaplayip VirusTotal'in (70+ antivirus motorunun ortak sonucunu tutan)
genel API'sine soruyoruz. Bu, gercek AV motorlarinin gercek zamanli imza
veritabanlarini kullanir -- kendi egittigimiz kucuk modelden kat kat
guvenilirdir.

Kullanim:
    export VT_API_KEY=senin_ucretsiz_api_anahtarin   # virustotal.com/gui/join-us
    python3 file_reputation.py /yol/programlar/kusku.exe

API anahtari yoksa: sadece hash hesaplanir ve nasil kontrol edilecegi
gosterilir; sahte bir sonuc UYDURULMAZ.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
import urllib.error
import urllib.request

VT_API_KEY = os.environ.get("VT_API_KEY", "")
VT_URL = "https://www.virustotal.com/api/v3/files/{sha256}"


def sha256_of(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def query_virustotal(sha256: str) -> dict | None:
    if not VT_API_KEY:
        return None
    req = urllib.request.Request(
        VT_URL.format(sha256=sha256),
        headers={"x-apikey": VT_API_KEY},
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"not_found": True}
        raise


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("Kullanim: python3 file_reputation.py <dosya-yolu>")
    path = sys.argv[1]
    if not os.path.isfile(path):
        sys.exit(f"Dosya bulunamadi: {path}")

    digest = sha256_of(path)
    print(f"Dosya      : {path}")
    print(f"SHA-256    : {digest}")

    if not VT_API_KEY:
        print(
            "\nVT_API_KEY tanimli degil -- gercek bir itibar sonucu UYDURULMUYOR.\n"
            "Ucretsiz anahtar almak icin: https://www.virustotal.com/gui/join-us\n"
            "Sonra: export VT_API_KEY=... ve bu betigi tekrar calistirin."
        )
        return

    result = query_virustotal(digest)
    if result is None:
        print("API anahtari okunamadi.")
        return
    if result.get("not_found"):
        print("Sonuc      : VirusTotal veritabaninda bu dosya YOK (daha once hic taranmamis).")
        print("             Bu 'temiz' anlamina gelmez -- bilinmiyor demektir.")
        return

    stats = result["data"]["attributes"]["last_analysis_stats"]
    malicious = stats.get("malicious", 0)
    suspicious = stats.get("suspicious", 0)
    total = sum(stats.values())
    print(f"Sonuc      : {malicious}/{total} motor zararli, {suspicious}/{total} motor supheli isaretledi")
    if malicious > 0:
        print("Karar      : ZARARLI - calistirmayin")
    elif suspicious > 0:
        print("Karar      : SUPHELI - dikkatli olun")
    else:
        print("Karar      : Taranan motorlarda temiz gorunuyor")


if __name__ == "__main__":
    main()

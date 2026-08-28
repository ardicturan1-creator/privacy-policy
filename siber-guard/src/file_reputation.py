#!/usr/bin/env python3
"""'Bu program virus mu?' sorusu icin dosya/URL itibar kontrolu.

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

v2 gelistirmeleri:
  - Yerel JSON onbellek (data/vt_cache.json): ayni dosya/URL tekrar tekrar
    sorulmaz -- hem VirusTotal'in ucretsiz kotasini (4 istek/dk) korur hem
    hizlandirir.
  - Toplu mod: birden fazla dosya tek komutla kontrol edilebilir.
  - URL itibar kontrolu: dosya disinda, bir web adresinin de VirusTotal
    tarafindan bilinen zararli/temiz durumu sorgulanabilir.
  - Basit, ACIKCA HEURISTIK (ML DEGIL) dosya-adi kontrolu: cift uzanti
    (ör. "fatura.pdf.exe") gibi klasik gizleme numaralarini yakalar --
    bu bir olasilik skoru degil, bilinen bir numarayi isaretleyen kural.

Kullanim:
    export VT_API_KEY=senin_ucretsiz_api_anahtarin   # virustotal.com/gui/join-us
    python3 file_reputation.py /yol/programlar/kusku.exe
    python3 file_reputation.py --batch dosya1.exe dosya2.zip
    python3 file_reputation.py --url https://supheli-site.top

API anahtari yoksa: sadece hash hesaplanir/heuristik calisir ve nasil
kontrol edilecegi gosterilir; sahte bir sonuc UYDURULMAZ.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import pathlib
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from config import ROOT, load_config  # noqa: E402

VT_API_KEY = os.environ.get("VT_API_KEY", "")
VT_FILE_URL = "https://www.virustotal.com/api/v3/files/{sha256}"
VT_URL_LOOKUP = "https://www.virustotal.com/api/v3/urls/{url_id}"

DOUBLE_EXTENSION_RISK = {
    ".exe", ".scr", ".bat", ".cmd", ".com", ".pif", ".vbs", ".js", ".jar",
}
DECOY_EXTENSIONS = {".pdf", ".doc", ".docx", ".xls", ".xlsx", ".jpg", ".png", ".txt"}


def sha256_of(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def double_extension_warning(path: str) -> str | None:
    """Heuristik (ML DEGIL): 'fatura.pdf.exe' gibi cift uzanti numarasi."""
    suffixes = [s.lower() for s in pathlib.Path(path).suffixes]
    if len(suffixes) >= 2 and suffixes[-1] in DOUBLE_EXTENSION_RISK and suffixes[-2] in DECOY_EXTENSIONS:
        return f"DIKKAT: dosya adi '{suffixes[-2]}' gibi gorunup aslinda '{suffixes[-1]}' calistirilabilir dosya -- klasik gizleme numarasi"
    return None


class VTCache:
    def __init__(self, path: pathlib.Path):
        self.path = path
        self.data: dict = {}
        if path.exists():
            try:
                self.data = json.loads(path.read_text(encoding="utf-8"))
            except (json.JSONDecodeError, OSError):
                self.data = {}

    def get(self, key: str, max_age_seconds: float = 6 * 3600) -> dict | None:
        entry = self.data.get(key)
        if entry and (time.time() - entry.get("_cached_at", 0)) < max_age_seconds:
            return entry
        return None

    def put(self, key: str, value: dict) -> None:
        value = dict(value)
        value["_cached_at"] = time.time()
        self.data[key] = value
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(self.data, indent=2, ensure_ascii=False), encoding="utf-8")


def _vt_get(url: str) -> dict | None:
    if not VT_API_KEY:
        return None
    req = urllib.request.Request(url, headers={"x-apikey": VT_API_KEY})
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"not_found": True}
        raise


def query_virustotal_file(sha256: str) -> dict | None:
    return _vt_get(VT_FILE_URL.format(sha256=sha256))


def query_virustotal_url(url: str) -> dict | None:
    # VirusTotal, URL kimligi olarak URL'nin base64url (dolgusuz) halini
    # bekler -- bkz. VT API v3 dokumantasyonu.
    url_id = base64.urlsafe_b64encode(url.encode()).decode().rstrip("=")
    return _vt_get(VT_URL_LOOKUP.format(url_id=url_id))


def summarize_stats(stats: dict) -> tuple[str, int, int, int]:
    malicious = stats.get("malicious", 0)
    suspicious = stats.get("suspicious", 0)
    total = sum(stats.values())
    if malicious > 0:
        verdict = "ZARARLI - calistirmayin/acmayin"
    elif suspicious > 0:
        verdict = "SUPHELI - dikkatli olun"
    else:
        verdict = "Taranan motorlarda temiz gorunuyor"
    return verdict, malicious, suspicious, total


def check_file(path: str, cache: VTCache) -> dict:
    if not os.path.isfile(path):
        return {"path": path, "error": f"Dosya bulunamadi: {path}"}

    digest = sha256_of(path)
    result = {"path": path, "sha256": digest}

    warn = double_extension_warning(path)
    if warn:
        result["heuristic_warning"] = warn

    if not VT_API_KEY:
        result["vt_status"] = "API_ANAHTARI_YOK"
        return result

    cached = cache.get(digest)
    if cached:
        result["vt_status"] = "onbellekten"
        vt_data = cached
    else:
        vt_data = query_virustotal_file(digest)
        if vt_data is None:
            result["vt_status"] = "API_HATASI"
            return result
        cache.put(digest, vt_data)
        result["vt_status"] = "canli_sorgu"

    if vt_data.get("not_found"):
        result["vt_verdict"] = "VirusTotal veritabaninda YOK (daha once hic taranmamis, 'temiz' anlamina gelmez)"
        return result

    stats = vt_data["data"]["attributes"]["last_analysis_stats"]
    verdict, malicious, suspicious, total = summarize_stats(stats)
    result.update({
        "vt_verdict": verdict,
        "vt_malicious": malicious,
        "vt_suspicious": suspicious,
        "vt_total_engines": total,
    })
    return result


def check_url(url: str, cache: VTCache) -> dict:
    result = {"url": url}
    if not VT_API_KEY:
        result["vt_status"] = "API_ANAHTARI_YOK"
        return result

    cache_key = f"url:{url}"
    cached = cache.get(cache_key)
    if cached:
        result["vt_status"] = "onbellekten"
        vt_data = cached
    else:
        vt_data = query_virustotal_url(url)
        if vt_data is None:
            result["vt_status"] = "API_HATASI"
            return result
        cache.put(cache_key, vt_data)
        result["vt_status"] = "canli_sorgu"

    if vt_data.get("not_found"):
        result["vt_verdict"] = "VirusTotal veritabaninda YOK (daha once hic taranmamis)"
        return result

    stats = vt_data["data"]["attributes"]["last_analysis_stats"]
    verdict, malicious, suspicious, total = summarize_stats(stats)
    result.update({
        "vt_verdict": verdict,
        "vt_malicious": malicious,
        "vt_suspicious": suspicious,
        "vt_total_engines": total,
    })
    return result


def print_result(result: dict) -> None:
    if "error" in result:
        print(f"HATA: {result['error']}")
        return
    if "path" in result:
        print(f"Dosya      : {result['path']}")
        print(f"SHA-256    : {result['sha256']}")
    if "url" in result:
        print(f"URL        : {result['url']}")
    if "heuristic_warning" in result:
        print(f"Uyari      : {result['heuristic_warning']}")

    status = result.get("vt_status")
    if status == "API_ANAHTARI_YOK":
        print(
            "VT_API_KEY tanimli degil -- gercek bir itibar sonucu UYDURULMUYOR.\n"
            "Ucretsiz anahtar almak icin: https://www.virustotal.com/gui/join-us\n"
            "Sonra: export VT_API_KEY=... ve bu betigi tekrar calistirin."
        )
    elif status == "API_HATASI":
        print("VirusTotal API'sine ulasilamadi.")
    elif "vt_verdict" in result:
        note = " (onbellekten)" if status == "onbellekten" else ""
        if "vt_total_engines" in result:
            print(f"Sonuc{note}   : {result['vt_malicious']}/{result['vt_total_engines']} motor zararli, "
                  f"{result['vt_suspicious']}/{result['vt_total_engines']} motor supheli")
        print(f"Karar      : {result['vt_verdict']}")
    print()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("path", nargs="?", help="Kontrol edilecek tek dosyanin yolu")
    ap.add_argument("--batch", nargs="+", metavar="DOSYA", help="Birden fazla dosya yolu")
    ap.add_argument("--url", help="Dosya yerine bir URL'in itibarini kontrol et")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    if not args.path and not args.batch and not args.url:
        sys.exit("Kullanim: python3 file_reputation.py <dosya-yolu> | --batch d1 d2 | --url <adres>")

    cfg = load_config()
    cache = VTCache(ROOT / cfg["vt_cache_path"])

    if args.url:
        result = check_url(args.url, cache)
        print(json.dumps(result, indent=2, ensure_ascii=False)) if args.json else print_result(result)
        return

    paths = args.batch if args.batch else [args.path]
    results = [check_file(p, cache) for p in paths]
    if args.json:
        print(json.dumps(results if args.batch else results[0], indent=2, ensure_ascii=False))
    else:
        for r in results:
            print_result(r)


if __name__ == "__main__":
    main()

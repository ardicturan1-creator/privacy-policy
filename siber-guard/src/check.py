#!/usr/bin/env python3
"""Domain/URL kontrolu icin komut satiri araci.

Tek domain (eskisiyle birebir uyumlu):
    python3 check.py trendyol.com

Toplu mod -- bir dosyadaki her satiri kontrol eder (ör. tarayici gecmisi
disa aktarimi, alan adi listesi):
    python3 check.py --batch domainler.txt

JSON cikti (baska araclarla entegrasyon icin):
    python3 check.py --json trendyol.com
    python3 check.py --batch domainler.txt --json
"""
from __future__ import annotations

import argparse
import json
import pathlib
import sys
from urllib.parse import urlparse

import joblib
import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from config import load_config  # noqa: E402
from features import extract_features, feature_vector  # noqa: E402

MODEL_PATH = pathlib.Path(__file__).resolve().parent.parent / "models" / "domain_classifier.joblib"


def load_model():
    if not MODEL_PATH.exists():
        sys.exit("Model bulunamadi. Once: python3 generate_dataset.py && python3 train_classifier.py")
    return joblib.load(MODEL_PATH)


def domain_from_input(value: str) -> str:
    if "://" in value:
        return urlparse(value).netloc
    return value.split("/")[0]


def build_reasons(feats: dict) -> list[str]:
    reasons = []
    if feats["has_suspicious_tld"]:
        reasons.append("supheli/ucuz uzanti (TLD) kullaniliyor")
    if feats["looks_like_brand_typo"]:
        reasons.append("bilinen bir markanin yazilisina typo ile benziyor")
    if feats["is_combosquat"]:
        reasons.append("bilinen bir marka adi, baska bir kelimeyle birlestirilmis (combosquat)")
    if feats["is_punycode"]:
        reasons.append("punycode/IDN kodlu -- gorsel olarak yaniltici karakter kullanimi olasi")
    if feats["has_suspicious_keyword"]:
        reasons.append("'login/verify/guvenlik' gibi supheli kelime iceriyor")
    if feats["is_ip_literal"]:
        reasons.append("domain yerine dogrudan IP adresi kullaniliyor")
    if feats["has_port"]:
        reasons.append("adreste standart-disi port numarasi belirtilmis")
    if feats["entropy"] > 3.5:
        reasons.append("rastgele/anlamsiz karakter dizisine benziyor (yuksek entropi)")
    if feats["is_regulated_suffix"]:
        reasons.append("resmi/denetimli bir uzanti kullaniyor (.gov.tr, .edu.tr vb.) -- guven artirici")
    return reasons


def verdict_for(risk: float, cfg: dict) -> str:
    threshold = cfg["risk_threshold"]
    if risk >= threshold:
        return "YUKSEK_RISK"
    if risk >= threshold * 0.65:
        return "SUPHELI"
    return "GUVENILIR"


VERDICT_TEXT = {
    "YUKSEK_RISK": "YUKSEK RISK - engellenmesi onerilir",
    "SUPHELI": "SUPHELI - dikkatli olun",
    "GUVENILIR": "GUVENILIR gorunuyor",
}


def evaluate(domain_input: str, clf, cfg: dict) -> dict:
    domain = domain_from_input(domain_input)
    x = np.array([feature_vector(domain)])
    risk = float(clf.predict_proba(x)[0][1])
    feats = extract_features(domain)
    verdict = verdict_for(risk, cfg)
    return {
        "input": domain_input,
        "domain": domain,
        "risk": round(risk, 4),
        "verdict": verdict,
        "verdict_text": VERDICT_TEXT[verdict],
        "reasons": build_reasons(feats),
    }


def print_human(result: dict) -> None:
    print(f"Domain     : {result['domain']}")
    print(f"Risk skoru : {result['risk']:.2f} (0=guvenli, 1=zararli)")
    print(f"Karar      : {result['verdict_text']}")
    if result["reasons"]:
        print("Gerekceler :")
        for r in result["reasons"]:
            print(f"  - {r}")
    print()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("domain", nargs="?", help="Kontrol edilecek tek domain/URL")
    ap.add_argument("--batch", metavar="DOSYA", help="Her satirinda bir domain/URL olan dosya")
    ap.add_argument("--json", action="store_true", help="JSON cikti ver")
    args = ap.parse_args()

    if not args.domain and not args.batch:
        sys.exit("Kullanim: python3 check.py <domain-veya-url>  |  python3 check.py --batch dosya.txt")

    cfg = load_config()
    bundle = load_model()
    clf = bundle["model"]

    if args.batch:
        lines = [
            line.strip() for line in pathlib.Path(args.batch).read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.strip().startswith("#")
        ]
        results = [evaluate(line, clf, cfg) for line in lines]
        if args.json:
            print(json.dumps(results, indent=2, ensure_ascii=False))
        else:
            for r in results:
                print_human(r)
            risky = [r for r in results if r["verdict"] != "GUVENILIR"]
            print(f"Ozet: {len(results)} domain kontrol edildi, {len(risky)} tanesi supheli/yuksek risk.")
        return

    result = evaluate(args.domain, clf, cfg)
    if args.json:
        print(json.dumps(result, indent=2, ensure_ascii=False))
    else:
        print_human(result)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Tek bir domain/URL icin komut satirindan hizli kontrol.

Kullanim:
    python3 check.py trendyol.com
    python3 check.py giris-trendyol-guvenlik.top
"""
from __future__ import annotations

import pathlib
import sys
from urllib.parse import urlparse

import joblib
import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from features import extract_features, feature_vector  # noqa: E402

MODEL_PATH = pathlib.Path(__file__).resolve().parent.parent / "models" / "domain_classifier.joblib"


def load_model():
    if not MODEL_PATH.exists():
        sys.exit("Model bulunamadi. Once: python3 src/generate_dataset.py && python3 src/train_classifier.py")
    return joblib.load(MODEL_PATH)


def domain_from_input(value: str) -> str:
    if "://" in value:
        return urlparse(value).netloc
    return value.split("/")[0]


def check(domain: str) -> None:
    bundle = load_model()
    clf = bundle["model"]
    domain = domain_from_input(domain)
    x = np.array([feature_vector(domain)])
    proba = clf.predict_proba(x)[0]
    risk = proba[1]

    if risk >= 0.75:
        verdict = "YUKSEK RISK - engellenmesi onerilir"
    elif risk >= 0.4:
        verdict = "SUPHELI - dikkatli olun"
    else:
        verdict = "GUVENILIR gorunuyor"

    print(f"Domain     : {domain}")
    print(f"Risk skoru : {risk:.2f} (0=guvenli, 1=zararli)")
    print(f"Karar      : {verdict}")

    feats = extract_features(domain)
    reasons = []
    if feats["has_suspicious_tld"]:
        reasons.append("supheli/ucuz uzanti (TLD) kullaniliyor")
    if feats["looks_like_brand_typo"]:
        reasons.append("bilinen bir markanin yazilisina typo ile benziyor")
    if feats["has_suspicious_keyword"]:
        reasons.append("'login/verify/guvenlik' gibi supheli kelime iceriyor")
    if feats["is_ip_literal"]:
        reasons.append("domain yerine dogrudan IP adresi kullaniliyor")
    if feats["entropy"] > 3.5:
        reasons.append("rastgele/anlamsiz karakter dizisine benziyor (yuksek entropi)")
    if reasons:
        print("Gerekceler :")
        for r in reasons:
            print(f"  - {r}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("Kullanim: python3 check.py <domain-veya-url>")
    check(sys.argv[1])

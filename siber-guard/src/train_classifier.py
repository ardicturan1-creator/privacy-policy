"""Hafif domain siniflandiricisi egitir (RandomForest, leksik ozelliklerle).

Model kucuk (birkac yuz KB) ve CPU'da milisaniyeler icinde tahmin yapar --
gercek zamanli DNS filtrelemeye uygun. Egitim de saniyeler surer (deep
learning degil, klasik ML -- bu is icin dogru arac: veri kucuk, ozellikler
elle tasarlanmis, yorumlanabilirlik onemli).
"""
from __future__ import annotations

import csv
import pathlib
import sys

import joblib
import numpy as np
from sklearn.ensemble import RandomForestClassifier
from sklearn.metrics import classification_report, confusion_matrix
from sklearn.model_selection import train_test_split

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from features import FEATURE_ORDER, feature_vector  # noqa: E402

HERE = pathlib.Path(__file__).resolve().parent
DATA_DIR = HERE.parent / "data"
MODEL_DIR = HERE.parent / "models"


def load_dataset() -> tuple[np.ndarray, np.ndarray, list[str]]:
    path = DATA_DIR / "dataset.csv"
    domains, labels = [], []
    with path.open(encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            domains.append(row["domain"])
            labels.append(int(row["label"]))
    X = np.array([feature_vector(d) for d in domains])
    y = np.array(labels)
    return X, y, domains


def main() -> None:
    X, y, domains = load_dataset()
    X_train, X_test, y_train, y_test, dom_train, dom_test = train_test_split(
        X, y, domains, test_size=0.25, random_state=42, stratify=y
    )

    clf = RandomForestClassifier(
        n_estimators=200,
        max_depth=10,
        min_samples_leaf=2,
        class_weight="balanced",
        random_state=42,
        n_jobs=-1,
    )
    clf.fit(X_train, y_train)

    pred = clf.predict(X_test)
    print("=== Test seti raporu ===")
    print(classification_report(y_test, pred, target_names=["guvenilir", "supheli/zararli"]))
    print("Karisiklik matrisi (satir=gercek, sutun=tahmin):")
    print(confusion_matrix(y_test, pred))

    print("\n=== Ozellik onemleri ===")
    for name, importance in sorted(zip(FEATURE_ORDER, clf.feature_importances_), key=lambda t: -t[1]):
        print(f"  {name:28s} {importance:.3f}")

    MODEL_DIR.mkdir(exist_ok=True)
    out_path = MODEL_DIR / "domain_classifier.joblib"
    joblib.dump({"model": clf, "feature_order": FEATURE_ORDER}, out_path)
    size_kb = out_path.stat().st_size / 1024
    print(f"\nModel kaydedildi: {out_path} ({size_kb:.1f} KB)")

    print("\n=== Yanlis siniflandirilan ornekler (test seti) ===")
    wrong = 0
    for dom, true, p in zip(dom_test, y_test, pred):
        if true != p:
            wrong += 1
            print(f"  {dom!r}: gercek={true} tahmin={p}")
    if wrong == 0:
        print("  (yok)")


if __name__ == "__main__":
    main()

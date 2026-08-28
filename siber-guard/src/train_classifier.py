"""Hafif domain siniflandiricisi egitir (leksik ozelliklerle).

v2 gelistirmeleri:
  - Tek train/test bolmesi yerine 5-kat stratified cross-validation
  - Birden fazla model aday (RandomForest, GradientBoosting, LogisticRegression)
    CV ile karsilastirilir, en iyisi secilir
  - Olasilik kalibrasyonu (CalibratedClassifierCV) -- check.py/dns_proxy.py
    risk skorlarinin daha guvenilir olmasi icin
  - ROC-AUC ve PR-AUC raporlanir (accuracy tek basina yaniltici olabilir)
  - Egitim uretecinden BAGIMSIZ "zor test seti" (hard_test_set.csv) uzerinde
    ayrica degerlendirme -- ilk surumdeki "%100 dogruluk" yanilticiligina
    karsi somut, durust bir olcum
  - Model dosyasina metadata eklenir (egitim tarihi, veri seti boyutu,
    CV metrikleri, secilen model adi) -- check.py/dns_proxy.py sadece
    bundle["model"] okudugu icin GERIYE DONUK UYUMLU (yeni alanlar eklendi,
    var olanlar degismedi)
  - Metin raporu models/evaluation_report.txt dosyasina da yazilir

Model kaydetme formati (models/domain_classifier.joblib) ayni kaliyor:
    {"model": <fitted estimator>, "feature_order": [...], ...ek metadata}
"""
from __future__ import annotations

import csv
import datetime
import json
import pathlib
import sys

import joblib
import numpy as np
from sklearn.calibration import CalibratedClassifierCV
from sklearn.ensemble import GradientBoostingClassifier, RandomForestClassifier
from sklearn.linear_model import LogisticRegression
from sklearn.metrics import (
    average_precision_score,
    classification_report,
    confusion_matrix,
    roc_auc_score,
)
from sklearn.model_selection import StratifiedKFold, cross_val_score, train_test_split
from sklearn.pipeline import Pipeline
from sklearn.preprocessing import StandardScaler

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from features import FEATURE_ORDER, feature_vector  # noqa: E402

HERE = pathlib.Path(__file__).resolve().parent
DATA_DIR = HERE.parent / "data"
MODEL_DIR = HERE.parent / "models"


def load_csv(path: pathlib.Path) -> tuple[np.ndarray, np.ndarray, list[str]]:
    domains, labels = [], []
    with path.open(encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            domains.append(row["domain"])
            labels.append(int(row["label"]))
    X = np.array([feature_vector(d) for d in domains])
    y = np.array(labels)
    return X, y, domains


def candidate_models() -> dict:
    return {
        "random_forest": RandomForestClassifier(
            n_estimators=200, max_depth=10, min_samples_leaf=2,
            class_weight="balanced", random_state=42, n_jobs=-1,
        ),
        "gradient_boosting": GradientBoostingClassifier(
            n_estimators=150, max_depth=3, learning_rate=0.1, random_state=42,
        ),
        "logistic_regression": Pipeline([
            ("scale", StandardScaler()),
            ("clf", LogisticRegression(max_iter=2000, class_weight="balanced", random_state=42)),
        ]),
    }


def select_best_model(X: np.ndarray, y: np.ndarray) -> tuple[str, object, dict]:
    cv = StratifiedKFold(n_splits=5, shuffle=True, random_state=42)
    results = {}
    print("=== 5-kat capraz dogrulama (egitim seti icinde) ===")
    for name, model in candidate_models().items():
        scores = cross_val_score(model, X, y, cv=cv, scoring="roc_auc")
        results[name] = {"roc_auc_mean": float(scores.mean()), "roc_auc_std": float(scores.std())}
        print(f"  {name:22s} ROC-AUC = {scores.mean():.4f} (+/- {scores.std():.4f})")

    best_name = max(results, key=lambda k: results[k]["roc_auc_mean"])
    print(f"En iyi model: {best_name}")
    return best_name, candidate_models()[best_name], results


def evaluate_on(clf, X: np.ndarray, y: np.ndarray, domains: list[str], label: str) -> dict:
    pred = clf.predict(X)
    proba = clf.predict_proba(X)[:, 1]
    report = classification_report(y, pred, target_names=["guvenilir", "supheli/zararli"], output_dict=True)
    cm = confusion_matrix(y, pred).tolist()
    roc_auc = roc_auc_score(y, proba) if len(set(y)) > 1 else float("nan")
    pr_auc = average_precision_score(y, proba) if len(set(y)) > 1 else float("nan")

    print(f"\n=== {label} ===")
    print(classification_report(y, pred, target_names=["guvenilir", "supheli/zararli"]))
    print(f"ROC-AUC: {roc_auc:.4f}   PR-AUC: {pr_auc:.4f}")
    print("Karisiklik matrisi (satir=gercek, sutun=tahmin):")
    print(np.array(cm))

    wrong = [(d, int(t), int(p)) for d, t, p in zip(domains, y, pred) if t != p]
    if wrong:
        print(f"Yanlis siniflandirilan {len(wrong)} ornek:")
        for d, t, p in wrong[:20]:
            print(f"  {d!r}: gercek={t} tahmin={p}")
    else:
        print("Yanlis siniflandirilan ornek yok.")

    return {
        "label": label,
        "n_samples": len(y),
        "roc_auc": roc_auc,
        "pr_auc": pr_auc,
        "confusion_matrix": cm,
        "report": report,
        "misclassified": wrong,
    }


def main() -> None:
    X, y, domains = load_csv(DATA_DIR / "dataset.csv")
    X_train, X_test, y_train, y_test, dom_train, dom_test = train_test_split(
        X, y, domains, test_size=0.25, random_state=42, stratify=y
    )

    best_name, best_model, cv_results = select_best_model(X_train, y_train)

    # Kalibrasyon: predict_proba ciktilarinin gercek olasiliga daha yakin
    # olmasini saglar (check.py/dns_proxy.py risk esigi bu skora dayaniyor).
    clf = CalibratedClassifierCV(best_model, method="isotonic", cv=5)
    clf.fit(X_train, y_train)

    holdout_metrics = evaluate_on(clf, X_test, y_test, dom_test, "Test seti (ayni uretecten, %25 ayrilmis)")

    hard_path = DATA_DIR / "hard_test_set.csv"
    hard_metrics = None
    if hard_path.exists():
        Xh, yh, domh = load_csv(hard_path)
        hard_metrics = evaluate_on(clf, Xh, yh, domh, "ZOR TEST SETI (bagimsiz, farkli uretim mantigi)")
    else:
        print("\n(hard_test_set.csv bulunamadi -- generate_dataset.py'yi calistirin)")

    # Ozellik onemleri: kalibre edilmis modelden degil, alttaki temel
    # tahminciden alinir (RandomForest/GBM icin dogrudan var; LR icinse
    # katsayi buyuklugu kullanilir).
    print("\n=== Ozellik onemleri (secilen model) ===")
    base = clf.calibrated_classifiers_[0].estimator
    if hasattr(base, "feature_importances_"):
        importances = base.feature_importances_
    elif hasattr(base, "named_steps"):
        importances = np.abs(base.named_steps["clf"].coef_[0])
        importances = importances / importances.sum()
    else:
        importances = np.zeros(len(FEATURE_ORDER))
    for name, importance in sorted(zip(FEATURE_ORDER, importances), key=lambda t: -t[1]):
        print(f"  {name:28s} {importance:.3f}")

    MODEL_DIR.mkdir(exist_ok=True)
    out_path = MODEL_DIR / "domain_classifier.joblib"
    joblib.dump({
        "model": clf,
        "feature_order": FEATURE_ORDER,
        "selected_model": best_name,
        "trained_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "train_size": len(X_train),
        "test_size": len(X_test),
        "cv_results": cv_results,
    }, out_path)
    size_kb = out_path.stat().st_size / 1024
    print(f"\nModel kaydedildi: {out_path} ({size_kb:.1f} KB), secilen: {best_name}")

    report_path = MODEL_DIR / "evaluation_report.txt"
    with report_path.open("w", encoding="utf-8") as f:
        f.write(f"Egitim zamani: {datetime.datetime.now(datetime.timezone.utc).isoformat()}\n")
        f.write(f"Secilen model: {best_name}\n\n")
        f.write("Capraz dogrulama (egitim seti icinde, 5-kat ROC-AUC):\n")
        f.write(json.dumps(cv_results, indent=2, ensure_ascii=False) + "\n\n")
        f.write(f"Test seti ROC-AUC: {holdout_metrics['roc_auc']:.4f}  PR-AUC: {holdout_metrics['pr_auc']:.4f}\n")
        f.write(f"Test seti yanlis siniflandirma sayisi: {len(holdout_metrics['misclassified'])}\n\n")
        if hard_metrics:
            f.write(f"ZOR TEST SETI ROC-AUC: {hard_metrics['roc_auc']:.4f}  PR-AUC: {hard_metrics['pr_auc']:.4f}\n")
            f.write(f"ZOR TEST SETI yanlis siniflandirma sayisi: {len(hard_metrics['misclassified'])}\n")
            f.write("ZOR TEST SETI yanlis siniflandirilan ornekler:\n")
            for d, t, p in hard_metrics["misclassified"]:
                f.write(f"  {d!r}: gercek={t} tahmin={p}\n")
    print(f"Degerlendirme raporu: {report_path}")


if __name__ == "__main__":
    main()

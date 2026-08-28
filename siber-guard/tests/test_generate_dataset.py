import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent / "src"))

from generate_dataset import (  # noqa: E402
    combosquat_variants,
    dga_variants,
    ip_literal_variants,
    punycode_variants,
    typosquat_variants,
)


def test_typosquat_variants_differ_from_brand():
    variants = typosquat_variants("trendyol", 10)
    assert len(variants) == 10
    assert all("trendyol.com" != v for v in variants)


def test_combosquat_contains_brand_unbroken():
    variants = combosquat_variants("trendyol", 10)
    assert len(variants) == 10
    for v in variants:
        registrable = v.split(".")[0]
        assert "trendyol" in registrable


def test_dga_variants_use_suspicious_tld():
    from generate_dataset import SUSPICIOUS_TLDS
    variants = dga_variants(10)
    assert len(variants) == 10
    for v in variants:
        tld = v.split(".")[-1]
        assert tld in SUSPICIOUS_TLDS


def test_ip_literal_variants_look_like_ip():
    import re
    variants = ip_literal_variants(5)
    assert len(variants) == 5
    for v in variants:
        assert re.fullmatch(r"(\d{1,3}\.){3}\d{1,3}", v)


def test_punycode_variants_have_prefix():
    variants = punycode_variants(5)
    assert len(variants) == 5
    for v in variants:
        assert v.startswith("xn--")


def test_no_overlap_between_benign_and_generated_malicious(tmp_path):
    """generate_dataset.build_dataset benign/malicious kumelerini ayirmali."""
    import csv

    import generate_dataset as gd

    out_dir = tmp_path
    (out_dir / "benign_domains.txt").write_text("trendyol.com\ngoogle.com\n", encoding="utf-8")
    original_data_dir = gd.DATA_DIR
    gd.DATA_DIR = out_dir
    try:
        gd.build_dataset()
        rows = list(csv.DictReader((out_dir / "dataset.csv").open(encoding="utf-8")))
        domains_by_label = {}
        for row in rows:
            domains_by_label.setdefault(row["label"], set()).add(row["domain"])
        assert domains_by_label["0"].isdisjoint(domains_by_label["1"])
        assert "trendyol.com" in domains_by_label["0"]
    finally:
        gd.DATA_DIR = original_data_dir

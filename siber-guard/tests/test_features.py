import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent / "src"))

from features import (  # noqa: E402
    extract_features,
    feature_vector,
    shannon_entropy,
    split_registrable_and_suffix,
)


def test_multi_part_tld_e_devlet():
    registrable, suffix, sub = split_registrable_and_suffix("e-devlet.gov.tr")
    assert registrable == "e-devlet"
    assert suffix == "gov.tr"
    assert sub == 0


def test_multi_part_tld_with_subdomain():
    registrable, suffix, sub = split_registrable_and_suffix("giris.e-devlet.gov.tr")
    assert registrable == "e-devlet"
    assert suffix == "gov.tr"
    assert sub == 1


def test_simple_tld_unaffected():
    registrable, suffix, sub = split_registrable_and_suffix("trendyol.com")
    assert registrable == "trendyol"
    assert suffix == "com"
    assert sub == 0


def test_ip_literal_detected():
    feats = extract_features("192.168.1.1")
    assert feats["is_ip_literal"] == 1


def test_suspicious_tld_flagged():
    feats = extract_features("giris-guvenlik.top")
    assert feats["has_suspicious_tld"] == 1


def test_regulated_suffix_flagged():
    feats = extract_features("e-devlet.gov.tr")
    assert feats["is_regulated_suffix"] == 1
    feats2 = extract_features("trendyol.com")
    assert feats2["is_regulated_suffix"] == 0


def test_combosquat_detected():
    feats = extract_features("trendyolindirimleri.com")
    assert feats["is_combosquat"] == 1


def test_exact_brand_is_not_combosquat():
    feats = extract_features("trendyol.com")
    assert feats["is_combosquat"] == 0


def test_punycode_detected():
    feats = extract_features("xn--pypal-4ve.com")
    assert feats["is_punycode"] == 1


def test_port_detected():
    feats = extract_features("ornek-site.com:8443")
    assert feats["has_port"] == 1


def test_entropy_random_string_higher_than_word():
    e1 = shannon_entropy("trendyol")
    e2 = shannon_entropy("xqzjklpmwv")
    assert e2 > e1


def test_feature_vector_matches_order_length():
    from features import FEATURE_ORDER
    vec = feature_vector("trendyol.com")
    assert len(vec) == len(FEATURE_ORDER)


def test_feature_vector_is_deterministic():
    assert feature_vector("trendyol.com") == feature_vector("trendyol.com")

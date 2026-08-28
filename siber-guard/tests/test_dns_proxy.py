import pathlib
import struct
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent / "src"))

from dns_proxy import (  # noqa: E402
    RiskCache,
    Stats,
    build_nxdomain,
    build_servfail,
    parse_qname,
)


def build_query(qname: str) -> bytes:
    tid = b"\xAB\xCD"
    flags = struct.pack(">H", 0x0100)
    counts = struct.pack(">HHHH", 1, 0, 0, 0)
    qparts = b"".join(bytes([len(label)]) + label.encode() for label in qname.split("."))
    question = qparts + b"\x00" + struct.pack(">HH", 1, 1)
    return tid + flags + counts + question


def test_parse_qname_roundtrip():
    q = build_query("giris-guvenlik.top")
    qname, _ = parse_qname(q, 12)
    assert qname == "giris-guvenlik.top"


def test_parse_qname_subdomain():
    q = build_query("a.b.example.com")
    qname, _ = parse_qname(q, 12)
    assert qname == "a.b.example.com"


def test_build_nxdomain_preserves_transaction_id():
    q = build_query("kotu-site.top")
    resp = build_nxdomain(q)
    assert resp[:2] == q[:2]
    rcode = resp[3] & 0x0F
    assert rcode == 3


def test_build_servfail_rcode():
    q = build_query("herhangi.com")
    resp = build_servfail(q)
    assert resp[:2] == q[:2]
    rcode = resp[3] & 0x0F
    assert rcode == 2


def test_risk_cache_hit_and_expiry():
    cache = RiskCache(ttl_seconds=100)
    assert cache.get("trendyol.com") is None
    cache.put("trendyol.com", 0.02)
    assert cache.get("trendyol.com") == 0.02
    assert len(cache) == 1


def test_risk_cache_expires(monkeypatch):
    cache = RiskCache(ttl_seconds=0.01)
    cache.put("trendyol.com", 0.02)
    import time
    time.sleep(0.05)
    assert cache.get("trendyol.com") is None


def test_stats_records_and_summarizes():
    stats = Stats()
    stats.record("kotu.top", blocked=True)
    stats.record("kotu.top", blocked=True)
    stats.record("iyi.com", blocked=False)
    assert stats.blocked == 2
    assert stats.allowed == 1
    assert "kotu.top" in stats.summary()

#!/usr/bin/env python3
"""Siber-Guard DNS filtreleme proxy'si (sinkhole).

Nasil calisir:
  1. Bu makine/router, DNS sunucusu olarak buradaki adresi kullanacak
     sekilde ayarlanir (127.0.0.1 veya bu sunucunun IP'si, port 5300).
  2. Cihaz bir domain coz(umle)mek istedikce (ör. tarayici bir siteye
     girerken) sorgu buraya gelir.
  3. Once yerel izin/red listesi (allowlist/denylist) kontrol edilir.
  4. Ardindan sorgulanan domain, egitilmis siniflandiricidan gecirilir
     (TTL'li bellek-ici onbellekle -- ayni domain tekrar tekrar
     siniflandirilmaz).
  5. Zararli/supheli bulunursa: NXDOMAIN donulur (baglanti kurulamaz).
     Guvenilir bulunursa: gercek DNS sunucusuna (varsayilan 1.1.1.1)
     yonlendirilir ve gercek cevap donulur.

Onemli sinirlama: Bu, TAM bir MITM/HTTPS-inceleme proxy'si DEGILDIR --
sadece hangi domain'e baglanilmak istendigini gorur ve engeller/izin verir.
Trafigin icerigini (HTTPS govdesini) okumaz. Bu bilincli bir tasarim
kararidir: icerik inceleme, her cihaza ozel bir kok sertifika kurulmasini
gerektirir ve ciddi guven/gizlilik riski tasir (bkz. README "Guvenlik
notlari"). DNS seviyesinde filtreleme, ev/kisisel kullanim icin Pi-hole
benzeri, kanitlanmis ve daha az riskli bir yontemdir.

Calistirma (eskisiyle birebir uyumlu):
    sudo python3 dns_proxy.py --listen 0.0.0.0 --port 5300 --upstream 1.1.1.1

CLI argumanlari verilmezse, siber-guard/config.json (varsa) ya da
config.py'deki varsayilanlar kullanilir -- varsayilanlar onceki surumle
aynidir, mevcut kullanim bicimi degismez.

(Standart DNS portu 53 root/yonetici yetkisi ister; test icin 5300 kullanin.)
"""
from __future__ import annotations

import argparse
import logging
import pathlib
import socket
import socketserver
import struct
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import joblib  # noqa: E402
import numpy as np  # noqa: E402
from config import ROOT, load_config, load_domain_list  # noqa: E402
from features import feature_vector  # noqa: E402

MODEL_PATH = pathlib.Path(__file__).resolve().parent.parent / "models" / "domain_classifier.joblib"

log = logging.getLogger("siber-guard")


def setup_logging(log_path: pathlib.Path) -> None:
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(message)s",
        handlers=[logging.FileHandler(log_path), logging.StreamHandler()],
    )


def load_model():
    if not MODEL_PATH.exists():
        sys.exit("Model bulunamadi. Once: python3 generate_dataset.py && python3 train_classifier.py")
    return joblib.load(MODEL_PATH)


def parse_qname(data: bytes, offset: int) -> tuple[str, int]:
    labels = []
    while True:
        length = data[offset]
        if length == 0:
            offset += 1
            break
        offset += 1
        labels.append(data[offset:offset + length].decode("ascii", errors="replace"))
        offset += length
    return ".".join(labels), offset


def build_nxdomain(query: bytes) -> bytes:
    tid = query[:2]
    flags = struct.pack(">H", 0x8183)  # QR=1 (yanit), RA=1, RCODE=3 (NXDOMAIN)
    counts = struct.pack(">HHHH", 1, 0, 0, 0)
    qname_end = query.find(b"\x00", 12) + 5
    question = query[12:qname_end]
    return tid + flags + counts + question


def build_servfail(query: bytes) -> bytes:
    """Upstream'e ulasilamadiginda donen yanit. Istemci hicbir zaman sessizce
    beklemeye birakilmaz -- sinirli sayida deneme sonrasi SERVFAIL (RCODE=2)
    donulur, boylece tarayici/isletim sistemi kendi yeniden deneme/hata
    mantigini calistirabilir."""
    tid = query[:2]
    flags = struct.pack(">H", 0x8182)  # QR=1, RA=1, RCODE=2 (SERVFAIL)
    counts = struct.pack(">HHHH", 1, 0, 0, 0)
    qname_end = query.find(b"\x00", 12) + 5
    question = query[12:qname_end]
    return tid + flags + counts + question


def forward_to_upstream(query: bytes, upstream: str, timeout: float = 2.0, retries: int = 2) -> bytes | None:
    """Upstream DNS sunucusuna sorguyu iletir. Sandbox/kisitlanmis ag
    ortamlarinda tek bir UDP paketi bazen kayboluyor -- bu yuzden sinirli
    sayida yeniden deneme var. Hepsi basarisiz olursa None doner (cagiran
    taraf SERVFAIL uretir); asla sinirsiz beklemez."""
    last_exc: Exception | None = None
    for _ in range(retries):
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
                s.settimeout(timeout)
                s.sendto(query, (upstream, 53))
                data, _ = s.recvfrom(4096)
                return data
        except OSError as exc:
            last_exc = exc
    if last_exc:
        log.error("upstream'e ulasilamadi (%d deneme): %s", retries, last_exc)
    return None


class RiskCache:
    """domain -> (risk, kayit_zamani) TTL'li kucuk onbellek.

    Ayni domain'e sik sorgu (ör. bir web sayfasindaki onlarca alt kaynak)
    her seferinde modeli yeniden calistirmaz -- gercek zamanli DNS
    yanitlama gecikmesini dusurur.
    """

    def __init__(self, ttl_seconds: float):
        self.ttl = ttl_seconds
        self._store: dict[str, tuple[float, float]] = {}

    def get(self, domain: str) -> float | None:
        entry = self._store.get(domain)
        if entry is None:
            return None
        risk, ts = entry
        if time.time() - ts > self.ttl:
            del self._store[domain]
            return None
        return risk

    def put(self, domain: str, risk: float) -> None:
        self._store[domain] = (risk, time.time())

    def __len__(self) -> int:
        return len(self._store)


class Stats:
    def __init__(self):
        self.allowed = 0
        self.blocked = 0
        self.blocked_domains: dict[str, int] = {}

    def record(self, domain: str, blocked: bool) -> None:
        if blocked:
            self.blocked += 1
            self.blocked_domains[domain] = self.blocked_domains.get(domain, 0) + 1
        else:
            self.allowed += 1

    def summary(self) -> str:
        top = sorted(self.blocked_domains.items(), key=lambda kv: -kv[1])[:5]
        top_str = ", ".join(f"{d}({c})" for d, c in top) or "yok"
        return f"izin={self.allowed} engel={self.blocked} en-cok-engellenen=[{top_str}]"


class DNSHandler(socketserver.BaseRequestHandler):
    model_bundle = None
    upstream = "1.1.1.1"
    risk_threshold = 0.6
    cache: RiskCache | None = None
    stats: Stats | None = None
    allowlist: set[str] = set()
    denylist: set[str] = set()

    def _is_listed(self, qname: str, listing: set[str]) -> bool:
        parts = qname.split(".")
        return any(".".join(parts[i:]) in listing for i in range(len(parts)))

    def handle(self) -> None:
        data, sock = self.request
        try:
            qname, _ = parse_qname(data, 12)
        except Exception:
            return
        if not qname:
            return

        if self._is_listed(qname, self.denylist):
            log.warning("ENGELLENDI (denylist) domain=%s", qname)
            self.stats.record(qname, blocked=True)
            sock.sendto(build_nxdomain(data), self.client_address)
            return

        if self._is_listed(qname, self.allowlist):
            log.info("izin verildi (allowlist) domain=%s", qname)
            self.stats.record(qname, blocked=False)
            self._forward(data, sock)
            return

        risk = self.cache.get(qname)
        if risk is None:
            clf = self.model_bundle["model"]
            x = np.array([feature_vector(qname)])
            risk = float(clf.predict_proba(x)[0][1])
            self.cache.put(qname, risk)

        if risk >= self.risk_threshold:
            log.warning("ENGELLENDI  risk=%.2f  domain=%s", risk, qname)
            self.stats.record(qname, blocked=True)
            sock.sendto(build_nxdomain(data), self.client_address)
            return

        log.info("izin verildi risk=%.2f  domain=%s", risk, qname)
        self.stats.record(qname, blocked=False)
        self._forward(data, sock)

    def _forward(self, data: bytes, sock) -> None:
        upstream_response = forward_to_upstream(data, self.upstream)
        if upstream_response is None:
            sock.sendto(build_servfail(data), self.client_address)
            return
        sock.sendto(upstream_response, self.client_address)


class ThreadingUDPServer(socketserver.ThreadingMixIn, socketserver.UDPServer):
    allow_reuse_address = True


def main() -> None:
    cfg = load_config()
    ap = argparse.ArgumentParser()
    ap.add_argument("--listen", default=cfg["dns_listen"])
    ap.add_argument("--port", type=int, default=cfg["dns_port"])
    ap.add_argument("--upstream", default=cfg["upstream_dns"], help="Gercek DNS sunucusu (guvenilir bulunan sorgular buraya gider)")
    ap.add_argument("--risk-threshold", type=float, default=cfg["risk_threshold"])
    ap.add_argument("--cache-ttl", type=float, default=cfg["cache_ttl_seconds"])
    args = ap.parse_args()

    setup_logging(ROOT / cfg["log_path"])

    DNSHandler.model_bundle = load_model()
    # Isinma cagrisi: sklearn/joblib bazi ic yapilari ilk predict_proba
    # cagrisinda kurar; bu maliyeti ilk gercek DNS sorgusuna degil,
    # burada baslangica yukleriz.
    DNSHandler.model_bundle["model"].predict_proba(np.array([feature_vector("warmup-check.com")]))
    DNSHandler.upstream = args.upstream
    DNSHandler.risk_threshold = args.risk_threshold
    DNSHandler.cache = RiskCache(args.cache_ttl)
    DNSHandler.stats = Stats()
    DNSHandler.allowlist = load_domain_list(cfg["allowlist_path"])
    DNSHandler.denylist = load_domain_list(cfg["denylist_path"])

    log.info(
        "Siber-Guard DNS proxy baslatiliyor: %s:%d (upstream=%s, esik=%.2f, "
        "onbellek-ttl=%.0fs, allowlist=%d, denylist=%d)",
        args.listen, args.port, args.upstream, args.risk_threshold,
        args.cache_ttl, len(DNSHandler.allowlist), len(DNSHandler.denylist),
    )
    server = ThreadingUDPServer((args.listen, args.port), DNSHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        log.info("Durduruldu. Ozet: %s", DNSHandler.stats.summary())


if __name__ == "__main__":
    main()

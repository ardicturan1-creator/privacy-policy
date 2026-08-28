#!/usr/bin/env python3
"""Siber-Guard DNS filtreleme proxy'si (sinkhole).

Nasil calisir:
  1. Bu makine/router, DNS sunucusu olarak buradaki adresi kullanacak
     sekilde ayarlanir (127.0.0.1 veya bu sunucunun IP'si, port 5300).
  2. Cihaz bir domain coz(umle)mek istedikce (ör. tarayici bir siteye
     girerken) sorgu buraya gelir.
  3. Sorgulanan domain, egitilmis siniflandiricidan gecirilir.
  4. Zararli/supheli bulunursa: NXDOMAIN donulur (baglanti kurulamaz).
     Guvenilir bulunursa: gercek DNS sunucusuna (varsayilan 1.1.1.1)
     yonlendirilir ve gercek cevap donulur.

Onemli sinirlama: Bu, TAM bir MITM/HTTPS-inceleme proxy'si DEGILDIR --
sadece hangi domain'e baglanilmak istendigini gorur ve engeller/izin verir.
Trafigin icerigini (HTTPS govdesini) okumaz. Bu bilincli bir tasarim
kararidir: icerik inceleme, her cihaza ozel bir kok sertifika kurulmasini
gerektirir ve ciddi guven/gizlilik riski tasir (bkz. README "Guvenlik
notlari"). DNS seviyesinde filtreleme, ev/kisisel kullanim icin Pi-hole
benzeri, kanitlanmis ve daha az riskli bir yontemdir.

Calistirma:
    sudo python3 dns_proxy.py --listen 0.0.0.0 --port 5300 --upstream 1.1.1.1

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
from features import feature_vector  # noqa: E402

MODEL_PATH = pathlib.Path(__file__).resolve().parent.parent / "models" / "domain_classifier.joblib"
LOG_PATH = pathlib.Path(__file__).resolve().parent.parent / "siber-guard.log"

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s %(levelname)s %(message)s",
    handlers=[logging.FileHandler(LOG_PATH), logging.StreamHandler()],
)
log = logging.getLogger("siber-guard")


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


def forward_to_upstream(query: bytes, upstream: str, timeout: float = 3.0) -> bytes:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
        s.settimeout(timeout)
        s.sendto(query, (upstream, 53))
        data, _ = s.recvfrom(4096)
        return data


class DNSHandler(socketserver.BaseRequestHandler):
    model_bundle = None
    upstream = "1.1.1.1"
    risk_threshold = 0.6

    def handle(self) -> None:
        data, sock = self.request
        try:
            qname, _ = parse_qname(data, 12)
        except Exception:
            return
        if not qname:
            return

        clf = self.model_bundle["model"]
        x = np.array([feature_vector(qname)])
        risk = clf.predict_proba(x)[0][1]

        if risk >= self.risk_threshold:
            log.warning("ENGELLENDI  risk=%.2f  domain=%s", risk, qname)
            response = build_nxdomain(data)
            sock.sendto(response, self.client_address)
            return

        log.info("izin verildi risk=%.2f  domain=%s", risk, qname)
        try:
            upstream_response = forward_to_upstream(data, self.upstream)
            sock.sendto(upstream_response, self.client_address)
        except OSError as exc:
            log.error("upstream hatasi domain=%s: %s", qname, exc)


class ThreadingUDPServer(socketserver.ThreadingMixIn, socketserver.UDPServer):
    allow_reuse_address = True


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--listen", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=5300)
    ap.add_argument("--upstream", default="1.1.1.1", help="Gercek DNS sunucusu (guvenilir bulunan sorgular buraya gider)")
    ap.add_argument("--risk-threshold", type=float, default=0.6)
    args = ap.parse_args()

    DNSHandler.model_bundle = load_model()
    DNSHandler.upstream = args.upstream
    DNSHandler.risk_threshold = args.risk_threshold

    log.info("Siber-Guard DNS proxy baslatiliyor: %s:%d (upstream=%s, esik=%.2f)",
              args.listen, args.port, args.upstream, args.risk_threshold)
    server = ThreadingUDPServer((args.listen, args.port), DNSHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        log.info("Durduruldu.")


if __name__ == "__main__":
    main()

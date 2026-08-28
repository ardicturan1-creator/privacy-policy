"""Merkezi ayar yonetimi.

Onceki surumde tum ayarlar (risk esigi, upstream DNS vb.) sadece komut
satiri argumaniyla veriliyordu. Bu surumde istege bagli bir
`siber-guard/config.json` dosyasi destekleniyor -- GERIYE DONUK UYUMLU:
dosya yoksa, aynen onceki surumdeki varsayilan degerler kullanilir, hicbir
mevcut komut satiri cagrisi bozulmaz. CLI argumanlari her zaman config
dosyasindaki degerleri ezer.
"""
from __future__ import annotations

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
CONFIG_PATH = ROOT / "config.json"

DEFAULTS = {
    "risk_threshold": 0.6,
    "upstream_dns": "1.1.1.1",
    "dns_listen": "127.0.0.1",
    "dns_port": 5300,
    "cache_ttl_seconds": 300,
    "allowlist_path": "data/allowlist.txt",
    "denylist_path": "data/denylist.txt",
    "log_path": "siber-guard.log",
    "vt_cache_path": "data/vt_cache.json",
}


def load_config() -> dict:
    cfg = dict(DEFAULTS)
    if CONFIG_PATH.exists():
        try:
            user_cfg = json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
            cfg.update({k: v for k, v in user_cfg.items() if k in DEFAULTS})
        except (json.JSONDecodeError, OSError):
            pass
    return cfg


def load_domain_list(relative_path: str) -> set[str]:
    path = ROOT / relative_path
    if not path.exists():
        return set()
    return {
        line.strip().lower()
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.strip().startswith("#")
    }

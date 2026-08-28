#!/usr/bin/env python3
"""siber-guard.log dosyasindan ozet rapor cikarir.

Kullanim:
    python3 stats.py                  # varsayilan log dosyasi
    python3 stats.py --last 100       # son 100 satiri incele
"""
from __future__ import annotations

import argparse
import pathlib
import re
import sys
from collections import Counter

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from config import ROOT, load_config  # noqa: E402

BLOCK_RE = re.compile(r"ENGELLENDI(?: \((\w+)\))?\s+(?:risk=([\d.]+)\s+)?domain=(\S+)")
ALLOW_RE = re.compile(r"izin verildi(?: \((\w+)\))?\s+(?:risk=([\d.]+)\s+)?domain=(\S+)")


def main() -> None:
    cfg = load_config()
    ap = argparse.ArgumentParser()
    ap.add_argument("--log", default=str(ROOT / cfg["log_path"]))
    ap.add_argument("--last", type=int, default=0, help="Sadece son N satiri incele (0 = tumu)")
    args = ap.parse_args()

    path = pathlib.Path(args.log)
    if not path.exists():
        sys.exit(f"Log dosyasi yok: {path} (dns_proxy.py henuz calistirilmadi mi?)")

    lines = path.read_text(encoding="utf-8").splitlines()
    if args.last:
        lines = lines[-args.last:]

    blocked = Counter()
    allowed = Counter()
    denylist_hits = 0
    allowlist_hits = 0

    for line in lines:
        m = BLOCK_RE.search(line)
        if m:
            source, _, domain = m.groups()
            blocked[domain] += 1
            if source == "denylist":
                denylist_hits += 1
            continue
        m = ALLOW_RE.search(line)
        if m:
            source, _, domain = m.groups()
            allowed[domain] += 1
            if source == "allowlist":
                allowlist_hits += 1

    total = sum(blocked.values()) + sum(allowed.values())
    print(f"Log dosyasi     : {path}")
    print(f"Incelenen satir : {len(lines)}")
    print(f"Toplam sorgu    : {total}")
    print(f"  izin verilen  : {sum(allowed.values())} ({allowlist_hits} allowlist ile)")
    print(f"  engellenen    : {sum(blocked.values())} ({denylist_hits} denylist ile)")

    if blocked:
        print("\nEn cok engellenen domainler:")
        for domain, count in blocked.most_common(10):
            print(f"  {count:5d}  {domain}")

    if total:
        block_rate = 100 * sum(blocked.values()) / total
        print(f"\nEngelleme orani: %{block_rate:.1f}")


if __name__ == "__main__":
    main()

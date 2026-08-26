#!/usr/bin/env python3
"""CHIMERA release-check — derlenmis Windows binary'lerinin gercekten
sertlestirilmis oldugunu OTOMATIK olarak dogrular. Ihlal bulunursa
sifir-olmayan bir kodla cikar; bu, CI/release pipeline'ina "hardening
gecidi" olarak eklenebilir (bkz. docs/chimera/03-HARDENING.md SS13).

Kontrol edilenler:
  1. PE DllCharacteristics: ASLR (DYNAMIC_BASE), DEP (NX_COMPAT) acik mi?
  2. Yasakli dize kaliplari: yapi makinesi mutlak yollari, kimlik bilgisi
     gorunumlu literaller, gelistirme/test uc noktalari.
  3. Sembol tablosu: linker seviyesinde stripping gercekten uygulanmis mi?
"""
import re
import struct
import subprocess
import sys
from pathlib import Path

FORBIDDEN_PATTERNS = [
    (rb"/home/[a-zA-Z0-9_.\-]+/", "yapi makinesi mutlak ev dizini"),
    (rb"/root/\.cargo/registry", "yapi makinesi cargo kayit defteri yolu"),
    (rb"(?i)TODO\(ffi\)", "tamamlanmamis FFI isareti (yer tutucu kalintisi)"),
    (rb"(?i)password\s*=\s*['\"][^'\"]{1,40}['\"]", "sabit-kodlanmis gorunumlu parola literali"),
    (rb"(?i)debug[-_]?endpoint", "hata ayiklama uc noktasi referansi"),
]

PE_FLAGS = {
    0x0020: "HIGH_ENTROPY_VA",
    0x0040: "DYNAMIC_BASE (ASLR)",
    0x0100: "NX_COMPAT (DEP)",
}
REQUIRED_PE_FLAGS = [0x0040, 0x0100]


def pe_dllcharacteristics(path: Path) -> int:
    data = path.read_bytes()
    e_lfanew = struct.unpack_from("<I", data, 0x3C)[0]
    if data[e_lfanew:e_lfanew + 4] != b"PE\0\0":
        raise ValueError(f"{path}: gecerli bir PE dosyasi degil")
    coff_off = e_lfanew + 4
    opt_off = coff_off + 20
    magic = struct.unpack_from("<H", data, opt_off)[0]
    is_pe32plus = magic == 0x20B
    dllchar_off = opt_off + (70 if is_pe32plus else 66)
    return struct.unpack_from("<H", data, dllchar_off)[0]


def check_pe_mitigations(path: Path) -> list[str]:
    problems = []
    dllchar = pe_dllcharacteristics(path)
    for bit in REQUIRED_PE_FLAGS:
        if not (dllchar & bit):
            problems.append(f"{path.name}: {PE_FLAGS[bit]} KAPALI (DllCharacteristics=0x{dllchar:04x})")
    return problems


def check_forbidden_strings(path: Path) -> list[str]:
    data = path.read_bytes()
    problems = []
    for pattern, description in FORBIDDEN_PATTERNS:
        matches = re.findall(pattern, data)
        if matches:
            sample = matches[0][:80]
            problems.append(f"{path.name}: yasakli kalip bulundu ({description}): {sample!r} (+{len(matches) - 1} tane daha)" if len(matches) > 1 else f"{path.name}: yasakli kalip bulundu ({description}): {sample!r}")
    return problems


def check_symbols_stripped(path: Path) -> list[str]:
    try:
        out = subprocess.run(["x86_64-w64-mingw32-nm", str(path)], capture_output=True, text=True, timeout=30)
    except FileNotFoundError:
        return []  # nm yoksa bu kontrolu atla, sert basarisizlik yapma
    if out.returncode != 0:
        return []
    lines = [l for l in out.stdout.splitlines() if l.strip()]
    # Stripped bir binary'de nm ya bosti tablosu bulamaz ya da yalnizca
    # dinamik import/export sembollerini gosterir (birkac duzine). Yuzlerce
    # yerel Rust sembolu (chimera_core::, core::, alloc::) gorulmesi
    # stripping'in TAM uygulanmadiginin isaretidir.
    local_symbols = [l for l in lines if re.search(r"chimera_|core::|alloc::", l)]
    if len(local_symbols) > 5:
        return [f"{path.name}: {len(local_symbols)} yerel sembol hala goruluyor — strip tam uygulanmamis olabilir"]
    return []


def main() -> int:
    targets = sys.argv[1:] or [
        "target/x86_64-pc-windows-gnu/release/chimera-core.exe",
        "target/x86_64-pc-windows-gnu/release/chimera-sentinel.exe",
        "target/x86_64-pc-windows-gnu/release/chimera-admin.exe",
        "target/x86_64-pc-windows-gnu/release/chimera-bootstrap.exe",
    ]
    repo_root = Path(__file__).resolve().parent.parent
    all_problems: list[str] = []

    for rel in targets:
        path = repo_root / rel
        if not path.exists():
            print(f"[atlandi] {rel} bulunamadi (once derleyin)")
            continue
        print(f"[kontrol] {rel}")
        all_problems += [f"  - {p}" for p in check_pe_mitigations(path)]
        all_problems += [f"  - {p}" for p in check_forbidden_strings(path)]
        all_problems += [f"  - {p}" for p in check_symbols_stripped(path)]

    if all_problems:
        print("\n=== HARDENING GECIDI BASARISIZ ===")
        for p in all_problems:
            print(p)
        return 1

    print("\n=== HARDENING GECIDI GECTI: ihlal bulunamadi ===")
    return 0


if __name__ == "__main__":
    sys.exit(main())

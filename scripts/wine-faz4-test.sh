#!/usr/bin/env bash
# CHIMERA Faz 4 — CANLI Wine dogrulama: surec sertlestirme, ETW, Windows Servisi.
set -u
SCRATCH=/tmp/chimera-faz4
export WINEDEBUG=-all WINEPREFIX=/tmp/chimera-wine
BIN="$(pwd)/target/x86_64-pc-windows-gnu/debug"
rm -rf "$SCRATCH"; mkdir -p "$SCRATCH"
WROOT="Z:${SCRATCH//\//\\}"
run() { timeout 120 wine "$BIN/$1.exe" "${@:2}" 2>/dev/null; }

CORE_PK=$(run chimera-core identity --root "$WROOT" | sed -n 's/^acik anahtar (hex): //p')
ADMIN_PK=$(run chimera-admin identity --root "$WROOT" | sed -n 's/^acik anahtar (hex): //p')
run chimera-core trust --root "$WROOT" --pubkey "$ADMIN_PK" >/dev/null
run chimera-admin trust-core --root "$WROOT" --pubkey "$CORE_PK" >/dev/null
PROV=$(run chimera-core provision --root "$WROOT")
S1=$(echo "$PROV" | sed -n 's/.*Pay A (TPM\/donanim tokenina): //p')
S2=$(echo "$PROV" | sed -n 's/.*Pay B (yonetici parolasina): //p')

echo "### 1) install-service (Wine SCM kismi -- sonuc DURUSTCE raporlanmali)"
run chimera-core install-service --root "$WROOT"; echo "  cikis: $?"

echo; echo "### 2) uninstall-service"
run chimera-core uninstall-service; echo "  cikis: $?"

echo; echo "### 3) serve -- sertlestirme + ETW durumu konsola yaziliyor mu?"
export CHIMERA_TARPIT_ADDR=127.0.0.1:0
timeout 60 wine "$BIN/chimera-core.exe" serve --root "$WROOT" >"$SCRATCH/serve.log" 2>&1 &
SERVE=$!
sleep 12
echo "--- serve.log ---"
cat "$SCRATCH/serve.log"

echo; echo "### 4) denetim kaydinda sertlestirme/ETW kayitlari:"
grep -oE '"event":"(mitigation|etw)[a-z._]*","detail":"[^"]{0,160}' "$SCRATCH/logs/audit.jsonl" 2>/dev/null | head -4

echo; echo "### 5) scan -- yeni bulgular"
run chimera-admin scan --root "$WROOT" --share "$S1" --share "$S2" | head -20

echo; echo "### 6) temiz durdurma"
kill -INT $SERVE 2>/dev/null; sleep 4
if kill -0 $SERVE 2>/dev/null; then echo "  HALA CALISIYOR"; kill -9 $SERVE; else echo "  temiz cikti"; fi

echo; echo "### 7) verify-audit"
run chimera-core verify-audit --root "$WROOT"

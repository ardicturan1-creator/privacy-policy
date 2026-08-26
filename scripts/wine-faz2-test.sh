#!/usr/bin/env bash
# CHIMERA Faz 2 — CANLI Wine dogrulama: 4625 brute-force tespiti,
# TTL'li oto-blok, coklu port tarpit.
set -u
SCRATCH="${SCRATCH:-/tmp/chimera-faz2}"
export WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-/tmp/chimera-wine}"
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
echo "### kurulum tamam"

# Tarpit'i ayricaliksiz portlara al (445/3389 Wine'da/kullanicida baglanamaz)
export CHIMERA_TARPIT_HOST=127.0.0.1
echo; echo "### serve baslatiliyor (coklu port tarpit denemesi dahil)"
timeout 90 wine "$BIN/chimera-core.exe" serve --root "$WROOT" >"$SCRATCH/serve.log" 2>&1 &
SERVE=$!
sleep 12
echo "--- serve.log ---"; sed -n '1,10p' "$SCRATCH/serve.log"

echo; echo "### 1) scan -- 4625 brute-force tespiti Wine'da ne diyor?"
run chimera-admin scan --root "$WROOT" --share "$S1" --share "$S2" | head -25

echo; echo "### 2) tarpit portlari GERCEKTEN dinliyor mu? (Linux tarafindan bakis)"
ss -tlnp 2>/dev/null | grep -E "445|3389" | head -5 || echo "(445/3389 dinlenmiyor -- ayricalikli port, beklenen)"

echo; echo "### 3) TTL oto-blok defteri (baslangicta bos olmali)"
cat "$SCRATCH/state/autoblock.list" 2>/dev/null || echo "(defter yok = hic otomatik engel uygulanmadi)"

echo; echo "### 4) never_block korumasi -- denetim kaydinda ret var mi?"
grep -o "autoblock.refused[^\"]*" "$SCRATCH/logs/audit.jsonl" 2>/dev/null | head -3 || echo "(ret kaydi yok)"

echo; echo "### 5) verify-audit"
run chimera-admin verify-audit --root "$WROOT" --share "$S1" --share "$S2"

kill -INT $SERVE 2>/dev/null; sleep 4
kill -9 $SERVE 2>/dev/null
echo; echo "### tarpit ile ilgili denetim kayitlari:"
grep -o "tarpit[a-z._]*" "$SCRATCH/logs/audit.jsonl" 2>/dev/null | sort | uniq -c
echo "### denetim kaydi satiri: $(wc -l < "$SCRATCH/logs/audit.jsonl" 2>/dev/null)"

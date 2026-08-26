#!/usr/bin/env bash
# CHIMERA Faz 3 — CANLI Wine dogrulama: imzali yedek akisi + bozulma tespiti.
set -u
SCRATCH=/tmp/chimera-faz3
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

mkdir -p "$SCRATCH/kullanici_belgeleri"
echo "cok onemli sozlesme" > "$SCRATCH/kullanici_belgeleri/sozlesme.docx"
head -c 3000 /dev/urandom > "$SCRATCH/kullanici_belgeleri/tablo.xlsx"
export CHIMERA_BACKUP_INCLUDE='Z:\tmp\chimera-faz3\kullanici_belgeleri'
export CHIMERA_TARPIT_ADDR=127.0.0.1:0

timeout 90 wine "$BIN/chimera-core.exe" serve --root "$WROOT" >"$SCRATCH/serve.log" 2>&1 &
SERVE=$!
sleep 12

echo "### 1) verify-backup -- HENUZ yedek yokken"
run chimera-admin verify-backup --root "$WROOT" --share "$S1" --share "$S2"; echo "  cikis: $?"

echo; echo "### 2) backup-now (Shamir 2/3) -- kullanici verisi DAHIL"
run chimera-admin backup-now --root "$WROOT" --share "$S1" --share "$S2"

echo; echo "### 3) diskte GERCEKTEN ne olustu?"
find "$SCRATCH/backups" -type f 2>/dev/null | sed "s|$SCRATCH/||" | sort | head -20
echo "--- manifest.txt ---"
cat "$SCRATCH"/backups/snapshot-*/manifest.txt 2>/dev/null | head -12
echo "--- imza uzunlugu (hex karakter): ---"
for f in "$SCRATCH"/backups/snapshot-*/manifest.sig; do [ -f "$f" ] && wc -c < "$f"; done

echo; echo "### 4) verify-backup -- SAGLAM olmali"
run chimera-admin verify-backup --root "$WROOT" --share "$S1" --share "$S2"; echo "  cikis: $?"

echo; echo "### 5) list-backups"
run chimera-admin list-backups --root "$WROOT" --share "$S1" --share "$S2"

echo; echo "### 6) SALDIRI: yedekteki KULLANICI dosyasi sessizce bozuluyor"
VICTIM=$(find "$SCRATCH/backups" -name "sozlesme.docx" | head -1)
chmod +w "$VICTIM" 2>/dev/null
echo "SALDIRGAN TARAFINDAN DEGISTIRILDI" > "$VICTIM"
echo "  bozulan: ${VICTIM#$SCRATCH/}"

echo; echo "### 7) verify-backup -- BOZULMA YAKALANMALI"
run chimera-admin verify-backup --root "$WROOT" --share "$S1" --share "$S2"; echo "  cikis: $?"

echo; echo "### 8) scan -- yedek sagligi / 4688 korlugu"
run chimera-admin scan --root "$WROOT" --share "$S1" --share "$S2" | grep -E "backup|cmdguard|bruteforce|TARAMA" | head -10

echo; echo "### 9) verify-audit"
run chimera-admin verify-audit --root "$WROOT" --share "$S1" --share "$S2"

kill -INT $SERVE 2>/dev/null; sleep 4; kill -9 $SERVE 2>/dev/null
echo; echo "### denetim kaydindaki yedek olaylari:"
grep -oE '"event":"backup\.[a-zA-Z_]*"' "$SCRATCH/logs/audit.jsonl" 2>/dev/null | sort | uniq -c

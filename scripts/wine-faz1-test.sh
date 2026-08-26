#!/usr/bin/env bash
# CHIMERA Faz 1 — CANLI Wine dogrulama betigi.
# Capraz derlenmis Windows ikililerini Wine altinda GERCEKTEN calistirir:
# identity -> trust -> provision -> serve -> list-suspended -> resume/terminate
# -> verify-audit -> temiz durdurma.
set -u
SCRATCH="${SCRATCH:-/tmp/chimera-faz1}"
export WINEDEBUG=-all
export WINEPREFIX="${WINEPREFIX:-/tmp/chimera-wine}"
BIN="$(pwd)/target/x86_64-pc-windows-gnu/debug"

rm -rf "$SCRATCH"; mkdir -p "$SCRATCH"
WROOT="Z:${SCRATCH//\//\\}"
echo "### KOK: $WROOT"

run() { timeout 120 wine "$BIN/$1.exe" "${@:2}" 2>/dev/null; }

echo; echo "### 1) core identity"
CORE_PK=$(run chimera-core identity --root "$WROOT" | sed -n 's/^acik anahtar (hex): //p')
echo "core pubkey uzunlugu: ${#CORE_PK}"

echo; echo "### 2) admin identity"
ADMIN_PK=$(run chimera-admin identity --root "$WROOT" | sed -n 's/^acik anahtar (hex): //p')
echo "admin pubkey uzunlugu: ${#ADMIN_PK}"

echo; echo "### 3) karsilikli guven"
run chimera-core trust --root "$WROOT" --pubkey "$ADMIN_PK" | head -1
run chimera-admin trust-core --root "$WROOT" --pubkey "$CORE_PK" | head -1

echo; echo "### 4) provision (Shamir 2/3)"
PROV=$(run chimera-core provision --root "$WROOT")
echo "$PROV" | grep -c "Pay" | xargs echo "uretilen pay sayisi:"
S1=$(echo "$PROV" | sed -n 's/.*Pay A (TPM\/donanim tokenina): //p')
S2=$(echo "$PROV" | sed -n 's/.*Pay B (yonetici parolasina): //p')

echo; echo "### 5) serve baslat (arka plan)"
timeout 90 wine "$BIN/chimera-core.exe" serve --root "$WROOT" >"$SCRATCH/serve.log" 2>&1 &
SERVE=$!
sleep 12
echo "serve.log:"; sed -n '1,8p' "$SCRATCH/serve.log"

echo; echo "### 6) status (ayricalik gerektirmez)"
run chimera-admin status --root "$WROOT"

echo; echo "### 7) list-suspended -- PAY YOK (KASA KAPALI olmali)"
run chimera-admin list-suspended --root "$WROOT"; echo "cikis kodu: $?"

echo; echo "### 8) list-suspended -- 2 GECERLI pay"
run chimera-admin list-suspended --root "$WROOT" --share "$S1" --share "$S2"

echo; echo "### 9) resume-process -- kuyrukta OLMAYAN pid (reddedilmeli)"
run chimera-admin resume-process --root "$WROOT" --share "$S1" --share "$S2" --pid 31337

echo; echo "### 10) terminate-process -- kuyrukta OLMAYAN pid (reddedilmeli)"
run chimera-admin terminate-process --root "$WROOT" --share "$S1" --share "$S2" --pid 31337

echo; echo "### 11) TUZAGA DOKUN (gercek fidye yazilimi taklidi)"
DECOY_DIR="$SCRATCH/decoys"
ls "$DECOY_DIR" 2>/dev/null | head -12
echo "--- alfabetik ilk 3 dosya (fidye yaziliminin ONCE gorecegi) ---"
ls "$DECOY_DIR" | sort | head -3
python3 -c "
import os,sys
d='$DECOY_DIR'
for name in sorted(os.listdir(d)):
    p=os.path.join(d,name)
    if os.path.isfile(p):
        open(p,'wb').write(os.urandom(4096))  # yuksek entropili UZERINE YAZMA
        print('sifrelendi:',name)
"
sleep 4

echo; echo "### 12) decoys (aldatma kaydi) -- PID alani var mi?"
run chimera-admin decoys --root "$WROOT" --share "$S1" --share "$S2" | head -c 900; echo

echo; echo "### 13) scan -- devre kesici kuyrugu bulguya yansiyor mu?"
run chimera-admin scan --root "$WROOT" --share "$S1" --share "$S2" | head -30

echo; echo "### 14) verify-audit (hash zinciri)"
run chimera-admin verify-audit --root "$WROOT" --share "$S1" --share "$S2"

echo; echo "### 15) temiz durdurma (SIGINT)"
kill -INT $SERVE 2>/dev/null
sleep 4
if kill -0 $SERVE 2>/dev/null; then echo "HALA CALISIYOR"; kill -9 $SERVE 2>/dev/null; else echo "temiz cikti"; fi
ls "$SCRATCH/runtime/" 2>/dev/null
echo; echo "### 16) yerel verify-audit"
run chimera-core verify-audit --root "$WROOT"
echo; echo "### denetim kaydi satir sayisi:"; wc -l < "$SCRATCH/logs/audit.jsonl" 2>/dev/null
echo "### aldatma kaydi satir sayisi:"; wc -l < "$SCRATCH/logs/deception.jsonl" 2>/dev/null

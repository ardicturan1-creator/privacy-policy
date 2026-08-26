#!/usr/bin/env bash
# Tarpit -> TTL oto-blok beslemesinin UCTAN UCA canli dogrulamasi.
set -u
SCRATCH=/tmp/chimera-faz2b
export WINEDEBUG=-all WINEPREFIX=/tmp/chimera-wine
BIN="$(pwd)/target/x86_64-pc-windows-gnu/debug"
rm -rf "$SCRATCH"; mkdir -p "$SCRATCH"
WROOT="Z:${SCRATCH//\//\\}"
run() { timeout 120 wine "$BIN/$1.exe" "${@:2}" 2>/dev/null; }

CORE_PK=$(run chimera-core identity --root "$WROOT" | sed -n 's/^acik anahtar (hex): //p')
ADMIN_PK=$(run chimera-admin identity --root "$WROOT" | sed -n 's/^acik anahtar (hex): //p')
run chimera-core trust --root "$WROOT" --pubkey "$ADMIN_PK" >/dev/null
run chimera-admin trust-core --root "$WROOT" --pubkey "$CORE_PK" >/dev/null
run chimera-core provision --root "$WROOT" >/dev/null

export CHIMERA_TARPIT_HOST=127.0.0.1
timeout 80 wine "$BIN/chimera-core.exe" serve --root "$WROOT" >"$SCRATCH/serve.log" 2>&1 &
SERVE=$!
sleep 12
echo "### serve.log:"; head -2 "$SCRATCH/serve.log"

echo; echo "### 3389 (sahte RDP) portuna GERCEK TCP baglantilari aciliyor..."
python3 - <<'PY'
import socket, time
for i in range(7):
    try:
        s = socket.create_connection(("127.0.0.1", 3389), timeout=3)
        data = s.recv(1)          # tarpit'in yavas akittigi ilk bayt
        print(f"  baglanti {i+1}: BASARILI, ilk bayt = {data!r}")
        s.close()
    except Exception as e:
        print(f"  baglanti {i+1}: HATA {e}")
    time.sleep(0.3)
PY

echo; echo "### 445 (sahte SMB) portuna 1 baglanti:"
python3 -c "
import socket
s=socket.create_connection(('127.0.0.1',445),timeout=3)
print('  445 BASARILI, ilk bayt =', repr(s.recv(1))); s.close()" 2>&1

sleep 3
echo; echo "### aldatma kaydi (tarpit_connect satirlari):"
grep -c tarpit_connect "$SCRATCH/logs/deception.jsonl" 2>/dev/null | xargs echo "  toplam kayit:"
head -3 "$SCRATCH/logs/deception.jsonl" 2>/dev/null

echo; echo "### denetim kaydindaki tarpit/autoblock olaylari:"
grep -oE '"event":"(tarpit|autoblock)[a-z._]*"' "$SCRATCH/logs/audit.jsonl" 2>/dev/null | sort | uniq -c

echo; echo "### oto-blok denemesinin AYRINTISI:"
grep -oE '"event":"(tarpit\.autoblock[a-z_]*|autoblock\.[a-z_]*)","detail":"[^"]*"' "$SCRATCH/logs/audit.jsonl" 2>/dev/null | head -3

kill -INT $SERVE 2>/dev/null; sleep 3; kill -9 $SERVE 2>/dev/null
echo; echo "### yerel verify-audit:"; run chimera-core verify-audit --root "$WROOT"

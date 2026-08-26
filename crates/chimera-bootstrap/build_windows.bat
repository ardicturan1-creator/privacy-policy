@echo off
REM CHIMERA / MONOLITH -- Windows derleme betigi.
REM
REM Bu bir "installer" degil, KAYNAKTAN GERCEK BIR .exe URETEN bir betiktir.
REM Depoda zaten cross-compile edilmis bir chimera-bootstrap.exe var, ama
REM kendi makinenizde DOGRULAMAK/yeniden uretmek isterseniz bu betigi
REM kullanin -- ayni kaynak koddan, sizin derleyicinizle, sizin gozunuzun
REM onunde derlenir.
REM
REM Gereksinim: Rust (https://rustup.rs). Baska hicbir sey gerekmez --
REM bu crate'in TUM bagimliliklari crates.io'dan saf Rust/derlenebilir
REM kutuphanelerdir (bkz. Cargo.toml basindaki not).

setlocal

where cargo >nul 2>nul
if errorlevel 1 (
    echo [HATA] Rust/cargo bulunamadi. Once https://rustup.rs adresinden kurun.
    exit /b 1
)

echo [1/2] cargo test  --  once GERCEK test paketi calistirilir
cargo test --release
if errorlevel 1 (
    echo [HATA] Testler basarisiz oldu -- derleme durduruldu.
    exit /b 1
)

echo [2/2] cargo build --release
cargo build --release
if errorlevel 1 (
    echo [HATA] Derleme basarisiz oldu.
    exit /b 1
)

echo.
echo Basarili: target\release\chimera-bootstrap.exe
echo.
echo Deneyin:
echo   target\release\chimera-bootstrap.exe probe
echo   target\release\chimera-bootstrap.exe install --root C:\chimera
echo   target\release\chimera-bootstrap.exe verify   --root C:\chimera
echo   target\release\chimera-bootstrap.exe obsidian-demo

endlocal

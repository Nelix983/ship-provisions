@echo off
REM Build the Windows executable ShipProvisions.exe (Rust version).
REM Requires: Windows 10/11 with the Rust toolchain installed (https://rustup.rs).
REM Double-click this file, or run it from a terminal.
setlocal
where cargo >nul 2>nul
if errorlevel 1 (
    echo Rust not found. Install it from https://rustup.rs and re-run this script.
    pause
    exit /b 1
)
cargo build --release
if errorlevel 1 (
    echo Build failed.
    pause
    exit /b 1
)
echo.
echo Build finished:  target\release\ShipProvisions.exe
echo Copy that single file to the ship's PC - double-click to run, no installation needed.
pause

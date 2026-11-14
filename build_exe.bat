@echo off
REM Build the Windows installer-ready executable ShipProvisions.exe
REM Requires: Windows 10/11 with Python 3.10+ installed (python on PATH).
REM Double-click this file, or run it from a terminal.
setlocal
where python >nul 2>nul
if errorlevel 1 (
    echo Python not found. Install Python 3.10+ from https://www.python.org/downloads/windows/
    echo Make sure "Add python.exe to PATH" is checked during installation.
    pause
    exit /b 1
)
python -m pip install --upgrade pip
python -m pip install -r requirements.txt pyinstaller
python -m PyInstaller ship_provisions.spec --noconfirm --clean
if errorlevel 1 (
    echo Build failed.
    pause
    exit /b 1
)
echo.
echo Build finished:  dist\ShipProvisions.exe
echo Share that single file with the ship - double-click to run, no installation needed.
pause

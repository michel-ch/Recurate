@echo off
rem Launch Recurate. Builds the release binary first if it is missing or stale.
setlocal
cd /d "%~dp0player"

where cargo >nul 2>&1
if errorlevel 1 (
    if exist "target\release\recurate.exe" (
        start "" "target\release\recurate.exe"
        exit /b 0
    )
    echo cargo not found on PATH and no prebuilt target\release\recurate.exe exists.
    echo Install Rust from https://rustup.rs and run this script again.
    pause
    exit /b 1
)

cargo build --release
if errorlevel 1 (
    echo Build failed.
    pause
    exit /b 1
)

start "" "target\release\recurate.exe"
endlocal

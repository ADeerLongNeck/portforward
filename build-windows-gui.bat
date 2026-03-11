@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

echo ==========================================
echo   Port Forward - Windows GUI Build
echo ==========================================
echo.

REM Check Node.js
where node >nul 2>&1
if %errorlevel% neq 0 (
    echo Error: Node.js is not installed.
    echo Please install from https://nodejs.org
    pause
    exit /b 1
)

REM Check Rust
where cargo >nul 2>&1
if %errorlevel% neq 0 (
    echo Error: Rust is not installed.
    echo Please install from https://rustup.rs
    pause
    exit /b 1
)

echo Installing dependencies...
call npm install
if %errorlevel% neq 0 (
    echo Error: npm install failed
    pause
    exit /b 1
)

echo.
echo Building Windows GUI...
echo.

call npm run tauri build
if %errorlevel% neq 0 (
    echo Error: Build failed
    pause
    exit /b 1
)

echo.
echo ==========================================
echo   Build Complete!
echo ==========================================
echo.

REM Show output files
set "MSI_PATH=src-tauri\target\release\bundle\msi"
set "EXE_PATH=src-tauri\target\release"

echo Output files:
echo.

if exist "%MSI_PATH%\*.msi" (
    for %%f in (%MSI_PATH%\*.msi) do (
        echo MSI: %%f
    )
)

if exist "%EXE_PATH%\port-forward.exe" (
    echo EXE: %EXE_PATH%\port-forward.exe
)

echo.
echo ==========================================
pause

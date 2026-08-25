@echo off
chcp 65001 >nul
REM Build script for text search - Windows version
REM Dual-process architecture:
REM   - Backend: Rust executable
REM   - Frontend: Electron application

echo ========================================
echo   Building text search application
echo ========================================

REM Get script directory as project root
set "PROJECT_DIR=%~dp0"
set "FRONTEND_DIR=%PROJECT_DIR%frontend\"

REM Get build mode parameter, default to release
set "BUILD_MODE=%~1"
if "%BUILD_MODE%"=="" set "BUILD_MODE=release"

if "%BUILD_MODE%"=="release" (
    set "BACKEND_PATH=%PROJECT_DIR%target\release\text_search.exe"
    set "CARGO_BUILD_CMD=cargo build --release --features with-http-server"
) else (
    set "BACKEND_PATH=%PROJECT_DIR%target\debug\text_search.exe"
    set "CARGO_BUILD_CMD=cargo build --features with-http-server"
)

echo.
echo ========================================
echo   Part 1: Building Rust Backend (%BUILD_MODE%)
echo ========================================

echo.
echo Step 1/3: Cleaning old build artifacts...
del /f /q "%PROJECT_DIR%target\debug\text_search.exe" 2>nul
del /f /q "%PROJECT_DIR%target\release\text_search.exe" 2>nul

echo.
echo Step 2/3: Compiling Rust backend (%BUILD_MODE%)...
cd /d "%PROJECT_DIR%"
%CARGO_BUILD_CMD%

echo.
echo ========================================
echo   Part 2: Building Electron Frontend
echo ========================================

echo.
echo Updating third-party license report...
cd /d "%PROJECT_DIR%"
cargo about --version >nul 2>&1
if errorlevel 1 (
    echo cargo-about not found; installing it...
    cargo install --locked --features cli cargo-about
    if errorlevel 1 exit /b 1
)
node "%PROJECT_DIR%scripts\generate_third_party_licenses.mjs"
if errorlevel 1 (
    echo ERROR: Third-party license report generation failed
    exit /b 1
)

echo.
echo Step 3/3: Building and packaging Electron application...
cd /d "%FRONTEND_DIR%"

REM Clean old frontend build artifacts
if exist "%FRONTEND_DIR%dist" rmdir /s /q "%FRONTEND_DIR%dist"
if exist "%FRONTEND_DIR%dist_electron" rmdir /s /q "%FRONTEND_DIR%dist_electron"

REM Copy backend executable to frontend/resources/backend for Electron packaging
echo Copying backend executable to frontend/resources/backend...
if not exist "%FRONTEND_DIR%resources\backend" mkdir "%FRONTEND_DIR%resources\backend"

REM Collect Cargo and npm dependency licenses for the packaged application.
if exist "%FRONTEND_DIR%resources\licenses" rmdir /s /q "%FRONTEND_DIR%resources\licenses"
mkdir "%FRONTEND_DIR%resources\licenses"
cd /d "%PROJECT_DIR%"
node "%PROJECT_DIR%scripts\collect_licenses.mjs" --output "%FRONTEND_DIR%resources\licenses"
if errorlevel 1 (
    echo ERROR: License collection failed
    exit /b 1
)
cd /d "%FRONTEND_DIR%"

if exist "%BACKEND_PATH%" (
    copy /y "%BACKEND_PATH%" "%FRONTEND_DIR%resources\backend\"
) else (
    echo ERROR: Backend executable not found at %BACKEND_PATH%
    exit /b 1
)

REM Package Electron application
call npm run electron:build

echo.
echo ========================================
echo   Build Complete!
echo ========================================
echo.
set "ELECTRON_APP_PATH=%FRONTEND_DIR%dist_electron\win-unpacked\text-search-electron.exe"
echo Output files:
echo   Backend: %BACKEND_PATH%
echo   Electron App: %ELECTRON_APP_PATH%
echo.

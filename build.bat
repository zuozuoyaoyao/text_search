@echo off
chcp 65001 >nul
setlocal
set "SCRIPT=%~dp0build.ps1"
set "MODE=%~1"

echo ========================================
echo   Text Search - Windows packaging
echo ========================================

if "%MODE%"=="" (
    echo Usage: %~nx0 [backend^|tauri^|all]
    echo.
    echo Modes:
    echo   backend   无 Tauri 版：text_search.exe（独立后端，内置 Web UI）
    echo   tauri     仅 Tauri 桌面版：text-search-tauri.exe
    echo   all       都包含
    echo.
    echo Examples:
    echo   %~nx0 backend    仅无 Tauri 版
    echo   %~nx0 tauri      仅 Tauri 版
    echo   %~nx0 all        都包含
    echo.
    echo Output: dist\TextSearch-v^<ver^>-win64[-backend^-|-tauri].zip
    echo.
    pause
    exit /b 0
)

if "%MODE%"=="--help" goto :showhelp
if "%MODE%"=="-h" goto :showhelp

powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" -Mode "%MODE%"
if errorlevel 1 (
    echo.
    echo Build FAILED. See messages above.
    echo.
    pause
    exit /b 1
)

echo.
pause
exit /b 0

:showhelp
powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" -Help
echo.
pause
exit /b 0

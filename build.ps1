# build.ps1 - One-click Windows packaging (produces a zip)
#   Modes (-Mode):
#     backend  无 Tauri 版：text_search.exe（独立后端，内置 Web UI）
#     tauri    仅 Tauri 桌面版：text-search-tauri.exe
#     all      都包含
#   No argument / --help / -h: show help.
#
# Output: dist\TextSearch-v<version>-win64[-backend|-tauri].zip

param(
    [Alias('h')]
    [switch]$Help,
    [ValidateSet('backend', 'tauri', 'all')]
    [string]$Mode
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

function Show-Help {
    $script = Split-Path -Leaf $MyInvocation.InvocationName
    @"
Usage: $script [-Mode <mode>]

Modes:
  backend   无 Tauri 版：text_search.exe（独立后端，内置 Web UI）
  tauri     仅 Tauri 桌面版：text-search-tauri.exe
  all       都包含

Options:
  -Mode <mode>   打包模式（缺省：无参数时显示本帮助）
  --help, -h     显示本帮助

Examples:
  $script                # 显示本帮助
  $script -Mode backend  # 仅无 Tauri 版
  $script -Mode tauri    # 仅 Tauri 版
  $script -Mode all      # 都包含

Output: dist\TextSearch-v<ver>-win64[-backend|-tauri].zip
"@
}

if ($Help -or -not $Mode) {
    Show-Help
    exit 0
}

function Step($msg) {
    Write-Host "`n==> $msg" -ForegroundColor Cyan
}

Step 'Updating third-party license report'
& cargo about --version *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Host 'cargo-about not found; installing it...' -ForegroundColor Yellow
    & cargo install --locked --features cli cargo-about
    if ($LASTEXITCODE -ne 0) { throw 'cargo-about installation failed' }
}
& node "$root\scripts\generate_third_party_licenses.mjs"
if ($LASTEXITCODE -ne 0) { throw 'third-party license report generation failed' }

Step 'Building frontend (vite)'
Push-Location "$root\frontend"
& npm run build
if ($LASTEXITCODE -ne 0) { throw 'frontend build failed' }
Pop-Location

# frontend/dist is embedded into the backend at compile time via include_dir!.
# Cargo does not track those files, so force a recompile so the binary contains
# the freshly built frontend.
Step 'Forcing backend recompile to embed latest frontend'
(Get-Item "$root\src\lib.rs").LastWriteTime = Get-Date

if ($Mode -eq 'tauri' -or $Mode -eq 'all') {
    Step 'Building backend and preparing Tauri sidecar'
    & node "$root\frontend\scripts\build-sidecar.mjs" --release
    if ($LASTEXITCODE -ne 0) { throw 'backend/sidecar build failed' }
} else {
    Step 'Building backend (text_search.exe)'
    & cargo build --release --features with-ws-server
    if ($LASTEXITCODE -ne 0) { throw 'backend build failed' }
}

$backendExe = "$root\target\release\text_search.exe"
$tauriExe = "$root\target\release\text-search-tauri.exe"

if ($Mode -eq 'tauri' -or $Mode -eq 'all') {
    Step 'Building Tauri desktop app'
    & npx --prefix frontend tauri build --no-bundle
    if ($LASTEXITCODE -ne 0) { throw 'tauri build failed' }
}

$version = (Get-Content "$root\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version
$suffix = if ($Mode -eq 'backend') { '-backend' } elseif ($Mode -eq 'tauri') { '-tauri' } else { '' }
$pkgName = "TextSearch-v$version-win64$suffix"
$staging = Join-Path $env:TEMP $pkgName
if (Test-Path $staging) { Remove-Item $staging -Recurse -Force }
New-Item -ItemType Directory -Path $staging | Out-Null

Step 'Staging files'
Copy-Item $backendExe $staging
if ($Mode -ne 'backend') { Copy-Item $tauriExe $staging }
Copy-Item "$root\README.md" "$staging\README.md"

Step 'Collecting dependency licenses'
New-Item -ItemType Directory -Path "$staging\licenses" | Out-Null
& node "$root\scripts\collect_licenses.mjs" --output "$staging\licenses"
if ($LASTEXITCODE -ne 0) { throw 'license collection failed' }

Step 'Creating zip'
$outDir = "$root\dist"
if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir | Out-Null }
$zip = "$outDir\$pkgName.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path "$staging\*" -DestinationPath $zip
Remove-Item $staging -Recurse -Force

Write-Host "`nDONE: $zip" -ForegroundColor Green

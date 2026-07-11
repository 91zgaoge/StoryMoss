# StoryMoss (草苔) 本地构建脚本
# 支持平台: Windows (当前主机), Linux (需 WSL), macOS (不可在 Windows 构建)
# 永久规则: 每次推送到 GitHub 前，必须先在本地执行此脚本构建

param(
    [switch]$Windows,
    [switch]$Linux,
    [switch]$All
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$frontendDir = Join-Path $projectRoot "src-frontend"
$tauriDir = Join-Path $projectRoot "src-tauri"

function Build-Windows {
    Write-Host "🪟 开始构建 Windows 版本..." -ForegroundColor Cyan
    Set-Location $frontendDir
    npm run build
    Set-Location $tauriDir
    cargo tauri build
    $msi = Get-ChildItem (Join-Path $projectRoot "src-tauri\target\release\bundle\msi\*.msi") -ErrorAction SilentlyContinue | Select-Object -First 1
    $nsis = Get-ChildItem (Join-Path $projectRoot "src-tauri\target\release\bundle\nsis\*.exe") -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($msi) { Write-Host "✅ Windows MSI: $($msi.FullName)" -ForegroundColor Green }
    if ($nsis) { Write-Host "✅ Windows NSIS: $($nsis.FullName)" -ForegroundColor Green }
}

function Build-Linux {
    Write-Host "🐧 检查 Linux 构建条件..." -ForegroundColor Cyan
    $wslList = wsl -l -v 2>$null | Select-String -Pattern "Ubuntu|Debian"
    if (-not $wslList) {
        Write-Host "⚠️ 未检测到可用的 WSL Linux 发行版，跳过 Linux 本地构建" -ForegroundColor Yellow
        Write-Host "   建议: wsl --install -d Ubuntu" -ForegroundColor DarkGray
        return
    }
    Write-Host "⚠️ Linux 本地构建需通过 WSL 执行，当前环境暂不支持自动交叉编译" -ForegroundColor Yellow
    Write-Host "   请手动在 WSL 中运行: cd $tauriDir ; cargo tauri build" -ForegroundColor DarkGray
}

function Build-MacOS {
    Write-Host "🍎 macOS 版本无法在 Windows 主机上本地构建" -ForegroundColor Yellow
    Write-Host "   请通过 GitHub Actions 或 macOS 主机构建" -ForegroundColor DarkGray
}

# 默认行为: 如果没有指定参数，构建 Windows
if (-not ($Windows -or $Linux -or $All)) {
    $Windows = $true
}

Set-Location $projectRoot

if ($All) {
    Build-Windows
    Build-Linux
    Build-MacOS
} else {
    if ($Windows) { Build-Windows }
    if ($Linux) { Build-Linux }
}

Set-Location $projectRoot
Write-Host "`n📦 本地构建完成" -ForegroundColor Cyan

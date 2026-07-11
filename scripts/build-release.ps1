#Requires -Version 5.1
<#
.SYNOPSIS
    StoryMoss 生产版本一键构建脚本
.DESCRIPTION
    执行完整生产构建流程：前端构建 → Rust release 编译 → MSI/NSIS 打包
    构建产物：.exe / .msi / -setup.exe
.PARAMETER SkipFrontend
    跳过前端构建（如果前端代码未修改）
.PARAMETER Target
    指定构建目标：exe(仅可执行文件) | msi | nsis | all(默认)
.EXAMPLE
    .\build-release.ps1
    .\build-release.ps1 -SkipFrontend
    .\build-release.ps1 -Target exe
#>
[CmdletBinding()]
param(
    [switch]$SkipFrontend,
    [ValidateSet('exe', 'msi', 'nsis', 'all')]
    [string]$Target = 'all'
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# 颜色输出辅助函数
function Write-Info($msg) { Write-Host "[INFO] $msg" -ForegroundColor Cyan }
function Write-Success($msg) { Write-Host "[OK] $msg" -ForegroundColor Green }
function Write-Warning($msg) { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Write-Error($msg) { Write-Host "[ERR] $msg" -ForegroundColor Red }

$projectRoot = Split-Path -Parent $PSScriptRoot
$frontendDir = Join-Path $projectRoot 'src-frontend'
$tauriDir = Join-Path $projectRoot 'src-tauri'
$startTime = Get-Date

Write-Info "StoryMoss v5.3.1 生产构建开始"
Write-Info "项目根目录: $projectRoot"
Write-Info "构建目标: $Target"

# ==================== Phase 1: 前端构建 ====================
if (-not $SkipFrontend) {
    Write-Info "Phase 1/3: 构建前端生产版本..."
    Set-Location $frontendDir
    
    # 检查 node_modules
    if (-not (Test-Path (Join-Path $frontendDir 'node_modules'))) {
        Write-Info "安装前端依赖..."
        npm install
        if ($LASTEXITCODE -ne 0) { throw "npm install 失败" }
    }
    
    npm run build
    if ($LASTEXITCODE -ne 0) { throw "前端构建失败" }
    Write-Success "前端构建完成"
} else {
    Write-Warning "跳过前端构建 (--SkipFrontend)"
}

# ==================== Phase 2: Rust Release 编译 ====================
Write-Info "Phase 2/3: Rust release 编译（首次编译可能需要 15-20 分钟）..."
Set-Location $tauriDir

# 设置环境变量加速编译
$env:RUST_LOG = 'info'

# 执行构建
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "Rust release 编译失败" }
Write-Success "Rust release 编译完成"

# ==================== Phase 3: 打包 ====================
Write-Info "Phase 3/3: 打包安装程序..."

# 构建产物路径
$exePath = Join-Path $tauriDir 'target\release\storymoss.exe'
$bundleDir = Join-Path $tauriDir 'target\release\bundle'

if ($Target -eq 'exe' -or $Target -eq 'all') {
    if (Test-Path $exePath) {
        $exeSize = [math]::Round((Get-Item $exePath).Length / 1MB, 2)
        Write-Success "可执行文件: $exePath (${exeSize}MB)"
    } else {
        Write-Error "未找到 .exe 文件"
    }
}

if ($Target -eq 'msi' -or $Target -eq 'all') {
    # 使用 tauri-bundler 构建 MSI
    Write-Info "构建 MSI 安装包..."
    cargo tauri bundle --bundles msi
    if ($LASTEXITCODE -ne 0) { Write-Warning "MSI 构建失败" }
    
    $msiFiles = Get-ChildItem (Join-Path $bundleDir 'msi') -Filter '*.msi' -ErrorAction SilentlyContinue
    if ($msiFiles) {
        foreach ($f in $msiFiles) {
            $size = [math]::Round($f.Length / 1MB, 2)
            Write-Success "MSI: $($f.FullName) (${size}MB)"
        }
    } else {
        Write-Warning "未找到 MSI 文件"
    }
}

if ($Target -eq 'nsis' -or $Target -eq 'all') {
    # 使用 tauri-bundler 构建 NSIS
    Write-Info "构建 NSIS 安装包..."
    cargo tauri bundle --bundles nsis
    if ($LASTEXITCODE -ne 0) { Write-Warning "NSIS 构建失败" }
    
    $nsisFiles = Get-ChildItem (Join-Path $bundleDir 'nsis') -Filter '*-setup.exe' -ErrorAction SilentlyContinue
    if ($nsisFiles) {
        foreach ($f in $nsisFiles) {
            $size = [math]::Round($f.Length / 1MB, 2)
            Write-Success "NSIS: $($f.FullName) (${size}MB)"
        }
    } else {
        Write-Warning "未找到 NSIS 安装包"
    }
}

# ==================== 完成统计 ====================
$endTime = Get-Date
$duration = $endTime - $startTime
Write-Host ""
Write-Success "构建完成！总耗时: $($duration.ToString('hh\:mm\:ss'))"
Write-Info "产物目录: $bundleDir"

# 显示日志系统信息
Write-Host ""
Write-Host "📋 日志系统信息" -ForegroundColor Magenta
Write-Host "  日志目录: %APPDATA%\storymoss\logs\" -ForegroundColor Gray
Write-Host "  日志级别: 生产环境 info，开发环境 debug" -ForegroundColor Gray
Write-Host "  日志保留: 7天自动清理，单文件10MB上限" -ForegroundColor Gray
Write-Host "  前端日志: warn/error 自动同步到后端日志文件" -ForegroundColor Gray

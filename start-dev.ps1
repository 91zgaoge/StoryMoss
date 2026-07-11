# StoryMoss (草苔) 开发环境启动脚本
# 自动启动前端和 Tauri 后端

param(
    [switch]$Build,
    [switch]$Clean
)

Write-Host "============================================" -ForegroundColor Cyan
Write-Host "  StoryMoss (草苔) 开发环境启动器" -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""

# 检查依赖
Write-Host "[1/4] 检查环境依赖..." -ForegroundColor Yellow

$nodeVersion = node --version 2>$null
if (-not $nodeVersion) {
    Write-Host "错误: Node.js 未安装。请安装 Node.js 18+" -ForegroundColor Red
    exit 1
}
Write-Host "  ✓ Node.js $nodeVersion" -ForegroundColor Green

$rustVersion = rustc --version 2>$null
if (-not $rustVersion) {
    Write-Host "错误: Rust 未安装。请安装 Rust 1.70+" -ForegroundColor Red
    exit 1
}
Write-Host "  ✓ Rust $rustVersion" -ForegroundColor Green

# 清理模式
if ($Clean) {
    Write-Host ""
    Write-Host "[清理] 清理缓存和依赖..." -ForegroundColor Yellow
    cargo clean
    if (Test-Path "src-frontend/node_modules") {
        Remove-Item -Recurse -Force "src-frontend/node_modules"
    }
}

# 安装依赖
Write-Host ""
Write-Host "[2/4] 检查并安装依赖..." -ForegroundColor Yellow

if (-not (Test-Path "src-frontend/node_modules")) {
    Write-Host "  安装前端依赖..." -ForegroundColor Gray
    Set-Location src-frontend
    npm install
    Set-Location ..
} else {
    Write-Host "  ✓ 前端依赖已安装" -ForegroundColor Green
}

# 构建模式
if ($Build) {
    Write-Host ""
    Write-Host "[3/4] 构建生产版本..." -ForegroundColor Yellow
    Set-Location src-frontend
    npm run build
    Set-Location ..
    Set-Location src-tauri
    cargo tauri build
    Set-Location ..
    Write-Host ""
    Write-Host "构建完成！安装包位于 src-tauri/target/release/bundle/" -ForegroundColor Green
    exit 0
}

# 检查端口占用
Write-Host ""
Write-Host "[3/4] 检查端口占用..." -ForegroundColor Yellow
$portInUse = netstat -ano | Select-String "5173" | Select-String "LISTENING"
if ($portInUse) {
    Write-Host "  端口 5173 被占用，尝试释放..." -ForegroundColor Yellow
    taskkill /F /IM node.exe 2>$null
    Start-Sleep -Seconds 2
}
Write-Host "  ✓ 端口可用" -ForegroundColor Green

# 启动开发服务器
Write-Host ""
Write-Host "[4/4] 启动开发服务器..." -ForegroundColor Yellow
Write-Host ""

# 使用 Start-Process 启动独立窗口
Write-Host "启动前端开发服务器..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "Set-Location src-frontend; npm run dev" -WindowStyle Normal

Write-Host "等待前端服务器启动..." -ForegroundColor Gray
Start-Sleep -Seconds 5

Write-Host "启动 Tauri 应用..." -ForegroundColor Cyan
Set-Location src-tauri
cargo tauri dev

Write-Host ""
Write-Host "应用已关闭。感谢使用 StoryMoss!" -ForegroundColor Green

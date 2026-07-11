# StoryMoss (草苔) 一键测试脚本
# 运行全部测试：Rust 后端 + 前端单元测试 + E2E

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot

function Test-Rust {
    Write-Host "`n🦀 Running Rust tests..." -ForegroundColor Cyan
    Push-Location (Join-Path $projectRoot "src-tauri")
    try {
        $output = cargo test 2>&1
        $output | ForEach-Object { Write-Host $_ }
        $passed = ($output | Select-String "test result: ok. (\d+) passed").Matches.Groups[1].Value
        Write-Host "✅ Rust tests passed: $passed" -ForegroundColor Green
    }
    catch {
        Write-Host "❌ Rust tests failed" -ForegroundColor Red
        Pop-Location
        exit 1
    }
    Pop-Location
}

function Test-Frontend {
    Write-Host "`n⚛️ Running frontend unit tests..." -ForegroundColor Cyan
    Push-Location (Join-Path $projectRoot "src-frontend")
    try {
        npm run test:run
        if ($LASTEXITCODE -ne 0) { throw "Frontend tests failed" }
        Write-Host "✅ Frontend tests passed" -ForegroundColor Green
    }
    catch {
        Write-Host "❌ Frontend tests failed" -ForegroundColor Red
        Pop-Location
        exit 1
    }
    Pop-Location
}

function Test-E2E {
    Write-Host "`n🎭 Running E2E tests..." -ForegroundColor Cyan
    Push-Location $projectRoot
    try {
        npx playwright test
        if ($LASTEXITCODE -ne 0) { throw "E2E tests failed" }
        Write-Host "✅ E2E tests passed" -ForegroundColor Green
    }
    catch {
        Write-Host "❌ E2E tests failed" -ForegroundColor Red
        Pop-Location
        exit 1
    }
    Pop-Location
}

# 主流程
$startTime = Get-Date

Test-Rust
Test-Frontend
# E2E 可选（需要前端 dev server 运行）
# Test-E2E

$duration = (Get-Date) - $startTime
Write-Host "`n✨ All tests completed in $($duration.ToString('mm\:ss'))" -ForegroundColor Green

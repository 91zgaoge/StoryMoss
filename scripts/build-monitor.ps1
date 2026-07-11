# 构建监控脚本 - 定期输出以保持后台任务心跳
$ErrorActionPreference = "Stop"

Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Starting cargo build --release..."

# 启动 cargo build 作为后台作业
$job = Start-Job -ScriptBlock {
    Set-Location "F:\mywork\CINEMA-AI\v2-rust\src-tauri"
    & cargo build --release 2>&1
}

# 监控作业进度
$lastOutputCount = 0
$rustcFound = $false
while ($job.State -eq "Running") {
    Start-Sleep -Seconds 30
    
    # 获取当前输出
    $output = Receive-Job -Job $job
    $newOutput = $output | Select-Object -Skip $lastOutputCount
    $lastOutputCount = $output.Count
    
    foreach ($line in $newOutput) {
        Write-Host $line
    }
    
    # 检查 rustc 进程
    $rustcProcs = Get-Process -Name "rustc" -ErrorAction SilentlyContinue
    if ($rustcProcs) {
        $duration = (Get-Date) - $rustcProcs[0].StartTime
        Write-Host "[$(Get-Date -Format 'HH:mm:ss')] rustc still running (linking)... elapsed: $($duration.ToString('hh\:mm\:ss'))"
        $rustcFound = $true
    } elseif ($rustcFound) {
        Write-Host "[$(Get-Date -Format 'HH:mm:ss')] rustc finished!"
        $rustcFound = $false
    } else {
        Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Waiting for build to start..."
    }
    
    # 检查 EXE 是否生成
    if (Test-Path "F:\mywork\CINEMA-AI\v2-rust\target\release\storymoss.exe") {
        $exe = Get-Item "F:\mywork\CINEMA-AI\v2-rust\target\release\storymoss.exe"
        Write-Host "[$(Get-Date -Format 'HH:mm:ss')] EXE found! Size: $($exe.Length) bytes"
    }
}

# 获取最终输出
$finalOutput = Receive-Job -Job $job -Keep
$newFinalOutput = $finalOutput | Select-Object -Skip $lastOutputCount
foreach ($line in $newFinalOutput) {
    Write-Host $line
}

# 检查构建结果
if (Test-Path "F:\mywork\CINEMA-AI\v2-rust\target\release\storymoss.exe") {
    $exe = Get-Item "F:\mywork\CINEMA-AI\v2-rust\target\release\storymoss.exe"
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] BUILD SUCCESS! EXE: $($exe.Length) bytes"
    exit 0
} else {
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] BUILD FAILED - No EXE found"
    exit 1
}

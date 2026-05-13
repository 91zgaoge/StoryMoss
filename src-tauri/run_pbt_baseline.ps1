$ErrorActionPreference = 'Continue'
$sw = [System.Diagnostics.Stopwatch]::StartNew()
cargo test --lib tests::bug_condition_v57 -- --nocapture
$sw.Stop()
Write-Host '---TIMING---'
Write-Host ("Elapsed: {0:N2}s" -f $sw.Elapsed.TotalSeconds)

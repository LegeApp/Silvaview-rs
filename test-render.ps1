# Test script to run SequoiaView-rs and capture diagnostic output
$ErrorActionPreference = "Stop"

# Set Rust logging to show all info and debug messages
$env:RUST_LOG = "sequoiaview_rs=debug"

Write-Host "Running SequoiaView-rs with diagnostic logging..." -ForegroundColor Green
Write-Host "Press Ctrl+C to stop" -ForegroundColor Yellow
Write-Host ""

# Run the program (will open a window)
& ".\target\release\sequoiaview-rs.exe" C:\ 2>&1 | Tee-Object -FilePath ".\render-log.txt"

Set-Location "D:\Rust-projects\SilvaView-rs"
.\target\release\debug-scan.exe "C:\" | Out-File -FilePath "mft-test-output.txt" -Encoding utf8

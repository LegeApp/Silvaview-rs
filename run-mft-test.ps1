Set-Location "D:\Rust-projects\SequoiaView-rs"
.\target\release\debug-scan.exe "C:\" | Out-File -FilePath "mft-test-output.txt" -Encoding utf8

$outputFile = "D:\Rust-projects\SilvaView-rs\scan-results.txt"
$exe = "D:\Rust-projects\SilvaView-rs\target\release\debug-scan.exe"
$args = "C:\"

# Run elevated and redirect output to file
Start-Process -Verb RunAs -FilePath $exe -ArgumentList $args -RedirectStandardOutput $outputFile -Wait -NoNewWindow

# Check size of various system directories

Write-Host "Checking C: drive space usage..."
Write-Host ""

$drive = Get-PSDrive C
$usedGB = [math]::Round($drive.Used/1GB, 2)
$freeGB = [math]::Round($drive.Free/1GB, 2)

Write-Host "Windows reports: $usedGB GB used, $freeGB GB free"
Write-Host ""

# Check various system locations
$locations = @(
    'C:\$Recycle.Bin',
    'C:\System Volume Information',
    'C:\Windows\WinSxS',
    'C:\ProgramData',
    'C:\hiberfil.sys',
    'C:\pagefile.sys',
    'C:\swapfile.sys'
)

foreach ($loc in $locations) {
    try {
        if (Test-Path $loc) {
            if ((Get-Item $loc -Force).PSIsContainer) {
                $size = (Get-ChildItem $loc -Force -Recurse -File -ErrorAction SilentlyContinue |
                        Measure-Object -Property Length -Sum).Sum
            } else {
                $size = (Get-Item $loc -Force).Length
            }

            if ($size -gt 0) {
                $sizeGB = [math]::Round($size/1GB, 2)
                Write-Host "$loc : $sizeGB GB"
            }
        }
    } catch {
        Write-Host "$loc : Access denied or error"
    }
}

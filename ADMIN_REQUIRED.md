# Administrator Privileges Required for C:\ Scanning

## Problem Summary

When you ran `sequoiaview-rs.exe "C:\"`, the scanner returned **0 entries**, resulting in:
- An empty tree (1 node = just the root)
- A blank gray visualization
- No meaningful data

## Root Cause

The **MFT (Master File Table) scanner** requires **Administrator privileges** to read the C:\ drive's file system metadata. Without admin rights:
1. MFT access is denied
2. Fallback to directory walking also fails due to system file permissions
3. Result: 0 files scanned

## Solution

### Option 1: Run as Administrator (Recommended for C:\)

**PowerShell:**
```powershell
# Right-click PowerShell → "Run as Administrator", then:
cd D:\Rust-projects\SequoiaView-rs
.\target\release\sequoiaview-rs.exe "C:\"
```

**Or use the helper script:**
```powershell
.\run-elevated.ps1
```

### Option 2: Scan a Non-System Directory (Works Without Admin)

```bash
# Scan your user directories or other drives
.\target\release\sequoiaview-rs.exe "D:\Rust-projects"
.\target\release\sequoiaview-rs.exe "C:\Users\YourName\Documents"
```

This works immediately without elevation and scans fine!

### Option 3: Validate Backend First

Before running the GUI, test if scanning works:
```bash
.\target\release\validate-backend.exe "C:\"
```

If you see `Entries: 0`, you need admin privileges.

## How to Tell if Scan Failed

Look for this log message:
```
ERROR Scan returned no files! Possible causes:
 - Scanning C:\ requires Administrator privileges
 - MFT access denied
 - Try scanning a different directory
 - Or run as Administrator
```

## Verified Working

✅ **D:\Rust-projects** - 15,155 files scanned successfully
✅ **Backend pipeline** - All stages pass
✅ **Rendering** - Cushion treemap with brighter colors
❌ **C:\ without admin** - 0 files (requires elevation)

## New Features in This Build

- **Brighter lighting** (40% ambient vs 23% before)
- **Lighter directory colors** (0.5 gray vs 0.3 before)
- **Clear error messages** when scan returns no data
- **All overlays disabled** for clean debugging
- **Text labels disabled** by default (clean visualization)

## Test Now

```bash
# Works immediately (no admin needed):
.\target\release\sequoiaview-rs.exe "D:\Rust-projects"

# Requires admin:
# Right-click PowerShell → Run as Administrator, then:
.\target\release\sequoiaview-rs.exe "C:\"
```

You should now see a **bright, colorful cushion treemap** with proper hierarchical shading!

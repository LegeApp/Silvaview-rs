# Administrator Privilege Elevation

SequoiaView-rs requires Administrator privileges for **fast MFT scanning** on NTFS drives.

## How It Works

### **Automatic UAC Prompt (Recommended)**

We've configured the app with an embedded manifest (`sequoiaview-rs.exe.manifest`) that requests admin privileges. When you run the `.exe`, Windows will automatically show the UAC dialog:

```
User Account Control
Do you want to allow this app to make changes to your device?

SequoiaView-rs
Verified publisher: (Unsigned)

[Yes] [No]
```

**Manifest settings:**
```xml
<requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
```

This is embedded at build time via `build.rs` using the `winres` crate.

---

## Build Configuration

### Files:
1. **`sequoiaview-rs.exe.manifest`** — XML manifest requesting admin
2. **`build.rs`** — Build script that embeds the manifest into the `.exe`
3. **`Cargo.toml`** — Includes `winres` as a build dependency

### Build Process:
```bash
cargo build --release
```

The resulting `target/release/sequoiaview-rs.exe` will have the manifest embedded. Double-clicking it will trigger the UAC prompt.

---

## Runtime Privilege Detection

The app detects its privilege level at runtime:

```rust
// src/scanner/elevation.rs
pub fn is_elevated() -> bool {
    // Checks TOKEN_ELEVATION via GetTokenInformation
}
```

**Behavior:**
- **If elevated (admin):** Uses MFT scanning (~3-5 seconds for C: drive)
- **If not elevated:** Falls back to jwalk (~30-60 seconds)

**Logs:**
```
INFO  Running with Administrator privileges - MFT scanning enabled!
```
or
```
WARN  Not running with Administrator privileges. MFT scanning unavailable.
INFO  For 10x faster scanning, run with 'Run as Administrator'
```

---

## Manual Elevation (Alternative)

If the user declines the UAC prompt or runs without elevation, they can manually re-run:

**Windows GUI:**
- Right-click `sequoiaview-rs.exe` → **Run as administrator**

**PowerShell:**
```powershell
Start-Process sequoiaview-rs.exe -Verb RunAs
```

**Command Prompt:**
```cmd
runas /user:Administrator sequoiaview-rs.exe
```

---

## Runtime Self-Elevation (Not Currently Used)

We've implemented `scanner::elevation::request_elevation()` which can programmatically re-launch the app with admin privileges via `ShellExecuteW` with the `runas` verb.

**To enable runtime elevation:**
Uncomment this line in `main.rs`:
```rust
if !scanner::elevation::is_elevated() {
    scanner::elevation::request_elevation()?;
}
```

This will:
1. Show UAC prompt when the app starts
2. Exit the current non-elevated process
3. Restart with admin privileges

**Trade-offs:**
- **Manifest approach (current):** Prompt before app even starts (better UX)
- **Runtime approach:** App starts, checks, then prompts (allows trying without admin first)

---

## Why Admin Privileges Are Needed

### **MFT Direct Parsing**

To scan NTFS drives fast, we need to:
1. Open raw volume handle: `\\.\C:`
2. Issue `FSCTL_GET_NTFS_VOLUME_DATA` ioctl
3. Read the Master File Table sequentially

**Windows Security:**
- Raw volume access requires `SeBackupPrivilege`
- Only granted to Administrators
- This bypasses NTFS filesystem layer → ~10x faster

**Without admin:**
- Uses standard `FindFirstFile`/`FindNextFile` APIs via `jwalk`
- Slower but works for any user

---

## Security Considerations

**Why we need it:**
- Reading raw disk sectors (not modifying anything)
- Legitimate use case: disk space analysis tools (WinDirStat, TreeSize, WizTree all require admin)

**What we DON'T do:**
- Write to disk
- Modify files
- Change system settings
- Install drivers/services

**User control:**
- User sees UAC prompt every time
- Can decline and still use the app (slower scanning)
- Fully transparent about privilege usage

---

## Testing

### **With Admin:**
```powershell
# Run from PowerShell as Administrator
cargo run --release
```

Expected log:
```
INFO  Running with Administrator privileges - MFT scanning enabled!
INFO  Opening NTFS volume: \\.\C:
INFO  MFT scan complete: 2000000 files, 500000 dirs, 250.00 GB in 3.45s
```

### **Without Admin:**
```powershell
# Run from normal PowerShell
cargo run --release
```

Expected log:
```
WARN  Not running with Administrator privileges. MFT scanning unavailable.
INFO  For 10x faster scanning, run with 'Run as Administrator'
INFO  Falling back to jwalk scanner
```

---

## Distribution

When distributing the `.exe`:

1. **Standalone `.exe`** with embedded manifest → auto-prompts UAC
2. **Installer (optional)** — NSIS/WiX can create shortcuts with `RunAsAdministrator` flag
3. **ClickOnce/MSIX** — can request privileges in deployment manifest

**Recommended approach:**
Ship the standalone `.exe` with the embedded manifest. Users will see the UAC prompt on first launch and can choose to allow or deny.

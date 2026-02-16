# Debug Scanner CLI Tool

A text-based debugging tool for testing the filesystem scanner without launching the GPU window.

## Build

```bash
cargo build --release --bin debug-scan
```

## Usage

```bash
# Scan C: drive (default)
cargo run --release --bin debug-scan

# Scan specific path
cargo run --release --bin debug-scan -- D:\MyFolder

# Scan with debug logging
RUST_LOG=debug cargo run --release --bin debug-scan

# Run the built executable directly
target\release\debug-scan.exe C:\
```

## Features

### 1. **Filesystem Scanning**
- Uses the same MFT/jwalk scanner as the GUI
- Shows real-time progress during scan
- Reports scan statistics

### 2. **File Type Analysis**
- Categorizes all files by type
- Shows count and total size per category
- Displays percentage of total disk usage

### 3. **Largest Files**
- Lists top 20 largest files
- Helps identify space hogs

### 4. **Tree Building Test**
- Verifies the arena tree construction works
- Tests that no stack overflow occurs

## Sample Output

```
═══════════════════════════════════════════════════════
  SilvaView-rs Debug Scanner
═══════════════════════════════════════════════════════

Scan path: C:\
Privileges: Administrator ✓ (MFT scanning enabled)

Starting scan...

Scanning: 157427 files, 37362 dirs, 183.45 GB

Scan complete!
  Files:     157427
  Dirs:      37362
  Total:     194789
  Size:      183.45 GB
  Duration:  10.24s

Building file tree...
Tree built: 194789 nodes

═══════════════════════════════════════════════════════
  File Type Analysis
═══════════════════════════════════════════════════════

Category             Files         Size  Percent
--------------------------------------------------
Code             43,521    32.45 GB     17.7%
Document         21,432    28.91 GB     15.8%
Archive           3,142    45.23 GB     24.7%
Image            12,583    18.34 GB     10.0%
Video               234    38.56 GB     21.0%
Audio             5,678     8.92 GB      4.9%
Executable        8,234     6.78 GB      3.7%
Other            62,603     4.26 GB      2.3%
--------------------------------------------------
TOTAL                      183.45 GB    100.0%

═══════════════════════════════════════════════════════
  Largest Files
═══════════════════════════════════════════════════════

 1. 8.5 GB - pagefile.sys
 2. 4.2 GB - Windows.iso
 3. 2.1 GB - movie.mkv
 4. 1.8 GB - database.sqlite
 5. 1.5 GB - backup.zip
...

Done!
```

## Use Cases

### **Verify MFT Scanner**
Check that file sizes and counts are reasonable:
```bash
cargo run --release --bin debug-scan -- C:\
```

Expected for typical Windows C: drive:
- **Files:** 100K - 500K
- **Size:** 50 GB - 500 GB
- **Duration:** 3-10 seconds (MFT), 30-60 seconds (jwalk)

**Red flags:**
- Size > 10 TB → MFT parsing bug
- Stack overflow → tree building bug
- Duration > 5 min → MFT not being used

### **Test Specific Directories**
Test on a small known directory:
```bash
cargo run --release --bin debug-scan -- D:\Projects
```

You can manually verify the output against Windows Explorer.

### **Benchmark Performance**
Compare MFT vs jwalk:

**With admin (MFT):**
```bash
cargo run --release --bin debug-scan -- C:\
# Should take ~3-10s
```

**Without admin (jwalk):**
```bash
# Run from non-elevated PowerShell
cargo run --release --bin debug-scan -- C:\
# Should take ~30-60s
```

### **Debug Crashes**
If the GUI crashes, the CLI will show where:
```bash
RUST_LOG=trace cargo run --bin debug-scan -- C:\
```

Possible crash points:
1. **During scan** → MFT parsing issue
2. **During "Building file tree"** → Stack overflow (should be fixed)
3. **During category analysis** → Tree corruption

## Troubleshooting

### Stack Overflow
If you see:
```
thread 'main' has overflowed its stack
```

This means the iterative tree building failed. Check:
- Deep directory nesting (> 500 levels?)
- Corrupted MFT data causing circular references

### Wrong File Sizes
If total size is > 1 PB or obviously wrong:

1. Check the log for:
   ```
   WARN Suspiciously large total size: 10794723.16 GB - likely MFT parsing error
   ```

2. This indicates the `FileNameAttribute.real_size` field is being read incorrectly

3. Try with a small folder first to isolate:
   ```bash
   cargo run --release --bin debug-scan -- C:\Windows\System32
   ```

### Slow Scanning
If MFT scanning is slow (> 30s):
1. Check privilege detection:
   ```
   Privileges: User (jwalk fallback)
   ```

2. Re-run as Administrator:
   ```powershell
   Start-Process -Verb RunAs "target\release\debug-scan.exe"
   ```

## Comparing with Windows Explorer

To verify accuracy:

1. Run debug-scan on a folder:
   ```bash
   cargo run --release --bin debug-scan -- "C:\Program Files"
   ```

2. Right-click "Program Files" in Explorer → Properties

3. Compare:
   - Size (should match within a few MB)
   - File count (should match exactly)

Note: Explorer's size includes filesystem overhead; we report raw file sizes.

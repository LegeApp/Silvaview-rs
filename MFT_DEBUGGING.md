# MFT Parsing Debug Notes

## Issue: Stack Overflow + Incorrect File Sizes

### Symptoms:
```
MFT scan complete: 157427 files, 37362 dirs, 10794723335.16 GB in 10.24s
thread '<unknown>' has overflowed its stack
```

**Analysis:**
1. **10,794,723 TB** is clearly wrong (that's ~11 exabytes)
2. Stack overflow during tree building

### Root Causes:

#### 1. Stack Overflow ✅ FIXED
**Problem:** Recursive `ensure_node()` with deep directory trees
**Solution:** Rewrote to iterative algorithm in `src/tree/mod.rs`

#### 2. Corrupted File Sizes 🔧 INVESTIGATING
**Problem:** `#[repr(C, packed)]` structs causing unaligned reads

**Possible issues:**
- The `FileNameAttribute` struct layout might not match the actual NTFS format
- `real_size` field offset is wrong
- We're reading garbage bytes as u64

### NTFS $FILE_NAME Attribute Layout (0x30)

According to NTFS spec:
```
Offset  Size  Field
------  ----  -----
0x00    8     Parent directory MFT reference
0x08    8     Creation time
0x10    8     Modification time
0x18    8     MFT modification time
0x20    8     Access time
0x28    8     Allocated size
0x30    8     Real size          ← We want this!
0x38    4     Flags
0x3C    4     Reparse value
0x40    1     Name length (in characters)
0x41    1     Namespace
0x42    2*N   Filename (UTF-16)
```

**Our struct:**
```rust
#[repr(C, packed)]
struct FileNameAttribute {
    parent_directory: u64,      // 0x00
    creation_time: u64,          // 0x08
    modification_time: u64,      // 0x10
    mft_modification_time: u64,  // 0x18
    access_time: u64,            // 0x20
    allocated_size: u64,         // 0x28
    real_size: u64,              // 0x30 ✓
    flags: u32,                  // 0x38
    reparse_value: u32,          // 0x3C
    name_length: u8,             // 0x40
    namespace: u8,               // 0x41
}
```

**Looks correct!** But `#[repr(C, packed)]` can cause issues with unaligned access on some platforms.

### Safer Approach: Manual Byte Reading

Instead of:
```rust
let fn_attr = unsafe { &*(record[value_offset..].as_ptr() as *const FileNameAttribute) };
file_size = fn_attr.real_size;
```

Do:
```rust
fn read_u64_le(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

file_size = read_u64_le(record, value_offset + 0x30);
```

### Next Steps:
1. ✅ Add sanity check (file_size > 1TB → 0)
2. ✅ Add warning log for suspiciously large totals
3. 🔲 Replace packed structs with manual byte reading
4. 🔲 Test on actual MFT data

### Testing:
Run with logging:
```
RUST_LOG=debug cargo run --release
```

Expected output for ~200GB C: drive:
```
MFT scan complete: ~500K files, ~100K dirs, ~200 GB in 3-5s
```

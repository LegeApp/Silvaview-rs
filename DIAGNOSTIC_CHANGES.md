# Diagnostic Changes for Bare-Bones Treemap Visualization

## Problem Identified

The application was only showing 6 gray areas because **the rendering code was skipping all directories**. It only rendered file nodes, which meant only the 6 files at the root of C:\ were visible.

## Changes Made

### 1. Fixed Rendering to Show ALL Nodes (src/render/scene.rs)

**Before:**
- Skipped all directory nodes with `if node.is_dir { continue; }`
- Only rendered file nodes
- Result: Only ~6 rectangles visible (files at root level)

**After:**
- Renders **both directories and files**
- Directories are rendered as gray (0.3, 0.3, 0.3)
- Files are colored by extension type
- Result: Full treemap showing all data

### 2. Added Comprehensive Diagnostic Logging

Added logging at key points to help debug:

- **src/tree/mod.rs**: Logs entry count (dirs + files) when building tree
- **src/app.rs**: Logs tree size and layout rect count during relayout
- **src/layout/squarify.rs**: Logs root node layout details
- **src/render/scene.rs**: Logs files + dirs rendered count

## How to Test

### Option 1: Run with Diagnostic Logging (Recommended)

```powershell
# Set logging environment variable
$env:RUST_LOG = "sequoiaview_rs=info"

# Run the app (must be elevated for MFT scanning)
.\target\release\sequoiaview-rs.exe C:\
```

Watch the console output. You should see:
```
Building tree from 500000 entries (50000 dirs, 450000 files)
Tree built: 500000 nodes
Computing layout for tree with 500000 nodes...
Layout computed: 75000 rectangles generated
Scene built: 60000 files + 15000 dirs = 75000 total rectangles rendered
```

### Option 2: Use the Test Script

```powershell
.\test-render.ps1
```

This will run with logging enabled and save output to `render-log.txt`.

## Expected Behavior Now

When you run the app as Administrator:

1. **Scan Phase**: MFT scan should find hundreds of thousands of files/dirs
2. **Tree Build**: Should create a tree with all those nodes
3. **Layout Phase**: Should compute rectangles for all visible nodes
4. **Render Phase**: Should render ALL rectangles (not just files)

**Visual Result**: You should see a FULL treemap with:
- Gray areas for directories
- Colored areas for files (by extension type)
- Cushion shading on all rectangles
- The entire C:\ drive visualized

## What This Achieves

This gives you the **bare-bones treemap** you requested:
- ✅ All data laid out as rectangles
- ✅ Proportional sizing (larger files/dirs = larger rectangles)
- ✅ Basic colors (gray for dirs, extension colors for files)
- ✅ Cushion shading for depth perception

## Next Steps for Visual Refinement

Once you verify the full treemap is displaying:

1. **Adjust colors** - Tweak the directory gray value or file colors
2. **Refine cushion shading** - Adjust height, light angle, etc.
3. **Add borders/padding** - Make rectangles more distinct
4. **Optimize rendering** - Only show files in deepest visible level
5. **Add labels** - Show file/folder names on larger rectangles
6. **Add UI overlay** - Top bar, tooltips, etc. (using Pure Vello approach)

## Diagnostic Log Levels

- `RUST_LOG=sequoiaview_rs=info` - Shows major steps (recommended)
- `RUST_LOG=sequoiaview_rs=debug` - Shows detailed layout info
- `RUST_LOG=sequoiaview_rs=trace` - Shows everything (very verbose)

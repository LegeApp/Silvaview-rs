# Text Label Fix

## Problem
The screenshot showed text labels completely obscuring the cushion treemap - hundreds of overlapping filenames making the visualization unusable.

## Cause
The text threshold was too low (1000px² = ~32x32 pixels), causing ~131 rectangles to show labels.

## Solution Applied

### 1. Text Labels Disabled by Default
```rust
pub show_text_labels: bool,  // Set to false in App::new()
```

### 2. When Enabled, Much More Selective
If you set `show_text_labels: true` in `src/app.rs`, labels only appear on:
- Rectangles > 100,000px² (vs old 1,000px²)
- Depth ≤ 2 (top 2 levels only)
- Width > 40px AND height > 12px (text actually fits)

This reduces labels from ~131 to **~5-10 maximum** (only the largest top-level folders).

## How to Toggle

**To Enable Limited Text Labels:**
Edit `src/app.rs` line ~82:
```rust
show_text_labels: true,  // Change from false
```

**Current Default (Clean Visualization):**
```rust
show_text_labels: false,  // No text, pure cushion treemap
```

## Recommended Approach

For the cleanest visualization matching the original SilvaView:
- Keep text labels **disabled** (current default)
- The cushion shading and colors alone should convey the hierarchy
- File/folder names can appear in:
  - Tooltip on hover (already implemented)
  - Breadcrumb at top (already implemented)
  - Analytics panel on right (already implemented)

The pure cushion treemap is often more readable than one cluttered with text!

# SequoiaView-rs: Modern Disk Space Visualization

## Vision

A GPU-accelerated disk space analyzer for Windows that renders cushion treemaps
in real-time using Vello. Scan multi-terabyte NTFS drives in seconds via direct
MFT parsing, with fallback to standard filesystem traversal for non-NTFS volumes.

---

## Phase 1: Foundation (MVP)

### 1.1 — Filesystem Scanner

**Goal:** Populate an in-memory file tree from disk as fast as possible.

| Strategy | Method | Speed | Privilege |
|----------|--------|-------|-----------|
| Primary  | Direct MFT parsing via `ntfs` crate + raw volume handle | ~3-5s for millions of files | Admin required |
| Fallback | Parallel walk via `jwalk` crate | ~30-60s for millions of files | User-level |

- Open raw volume `\\.\C:` using the `windows` crate
- Parse MFT records sequentially, extract `$FILE_NAME` (0x30) and `$DATA` (0x80) attributes
- Reconstruct parent-child hierarchy using MFT record numbers
- Compute directory sizes by bottom-up aggregation
- Auto-detect NTFS vs other filesystems; fall back to `jwalk` when MFT unavailable

### 1.2 — Data Model (Arena Tree)

**Goal:** Cache-friendly, flat tree structure that scales to 10M+ nodes.

```rust
/// Index into the arena Vec
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

pub struct FileNode {
    pub name: CompactString,       // interned or small-string-optimized
    pub size: u64,                 // bytes (file) or aggregated (dir)
    pub is_dir: bool,
    pub extension_id: u16,         // index into extension table
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
}

pub struct FileTree {
    pub nodes: Vec<FileNode>,       // arena — contiguous memory
    pub root: NodeId,
    pub extensions: Vec<String>,    // deduplicated extension table
}
```

- Sibling-list tree: each node stores `first_child` + `next_sibling`
- Extension table for O(1) file-type grouping and color lookup
- `CompactString` or inline [u8; 22] for short names to avoid heap allocs

### 1.3 — Squarified Treemap Layout

**Goal:** Compute pixel-space rectangles for all visible nodes.

```rust
pub struct LayoutRect {
    pub node: NodeId,
    pub x: f32, pub y: f32,
    pub w: f32, pub h: f32,
    pub depth: u8,               // nesting depth for cushion calculation
}
```

Algorithm (Bruls, Huizing, van Wijk 2000):
1. Sort children by size descending
2. Greedily pack into rows, minimizing worst aspect ratio
3. Recurse into each child directory
4. Stop recursion when rect area < threshold (LOD culling)

Parallelism: use `rayon` to layout independent subtrees concurrently.

### 1.4 — Vello Rendering Pipeline

**Goal:** Draw treemap with cushion shading at 60fps.

**Tech stack:**
- `wgpu` — GPU abstraction (Vulkan/DX12/Metal)
- `vello` — 2D scene encoding (compute-shader rasterizer)
- `winit` — cross-platform windowing + event loop

**Rendering approach:**

For MVP, approximate cushion shading using Vello's gradient primitives:
- Each file rectangle → filled rect with a **radial or linear gradient**
  that simulates the parabolic cushion illumination
- Color hue determined by file extension category
- Gradient intensity modulated by nesting depth (deeper = flatter cushion)

For Phase 2, implement true per-pixel cushion shading via custom WGSL shader:
- Pass rectangle bounds + depth as uniforms
- Compute parabolic surface normal per pixel
- Apply Lambertian lighting: `I = max(dot(N, L), 0.0)`

**Scene construction each frame:**
```
for rect in visible_layout_rects {
    let color = extension_color(rect.node);
    let gradient = cushion_gradient(rect, depth);
    scene.fill(Fill::NonZero, transform, gradient, None, &rect_path);
}
```

### 1.5 — Basic Interaction

- **Hover:** highlight rectangle, show tooltip (filename, size, path)
- **Click:** drill down into directory (set new layout root)
- **Right-click / Backspace:** navigate up one level
- **Scroll wheel:** zoom in/out with smooth animation
- **Resize:** responsive relayout

---

## Phase 2: Polish & Advanced Features

### 2.1 — True Cushion Shader (WGSL)
- Custom fragment shader for per-pixel parabolic illumination
- Adjustable light direction (drag to change)
- Adjustable cushion height parameter (flat ↔ deep)

### 2.2 — File Type Analytics Panel
- Side panel showing breakdown by extension category
- Bar chart or pie chart of space per type
- Click category to highlight all matching files in treemap

### 2.3 — USN Journal Live Updates
- Monitor NTFS change journal for real-time file events
- Animate treemap changes (grow/shrink rectangles)
- `usn-journal-rs` or manual DeviceIoControl calls

### 2.4 — Glassmorphism UI
- Translucent overlay panels with backdrop blur
- Dark mode with vibrant file-type color palette
- Smooth zoom/drill-down transitions

### 2.5 — Multi-Drive Support
- Drive picker on launch
- Scan multiple volumes concurrently
- Tabbed or split view

---

## Project Structure

```
SequoiaView-rs/
├── Cargo.toml
├── PLAN.md                          # this file
├── src/
│   ├── main.rs                      # entry point, winit event loop
│   ├── app.rs                       # application state, frame orchestration
│   │
│   ├── scanner/
│   │   ├── mod.rs                   # Scanner trait, drive detection
│   │   ├── mft.rs                   # NTFS MFT direct parser (admin)
│   │   ├── walk.rs                  # jwalk-based fallback scanner
│   │   └── types.rs                 # RawFileEntry, ScanProgress
│   │
│   ├── tree/
│   │   ├── mod.rs                   # FileTree, NodeId
│   │   ├── arena.rs                 # Arena allocator, tree construction
│   │   ├── aggregate.rs             # Bottom-up size computation
│   │   └── extensions.rs            # Extension table, category mapping
│   │
│   ├── layout/
│   │   ├── mod.rs                   # LayoutRect, layout orchestration
│   │   └── squarify.rs              # Squarified treemap algorithm
│   │
│   ├── render/
│   │   ├── mod.rs                   # Renderer setup (wgpu + vello)
│   │   ├── scene.rs                 # Vello scene builder (rects + gradients)
│   │   ├── cushion.rs               # Cushion gradient computation
│   │   └── colors.rs                # Extension → color mapping
│   │
│   └── ui/
│       ├── mod.rs                   # UI state machine
│       ├── input.rs                 # Mouse/keyboard event handling
│       ├── navigation.rs            # Zoom, pan, drill-down logic
│       └── tooltip.rs               # Hover tooltip rendering
│
└── shaders/                         # Phase 2
    └── cushion.wgsl                 # Per-pixel cushion fragment shader
```

---

## Key Dependencies

```toml
[dependencies]
# GPU rendering
vello = "0.4"                        # compute-shader 2D renderer
wgpu = "24"                          # GPU abstraction
winit = "0.30"                       # windowing

# Filesystem
jwalk = "0.8"                        # parallel directory walker
ntfs = "0.4"                         # NTFS/MFT parser

# Windows APIs
windows = { version = "0.58", features = [
    "Win32_Storage_FileSystem",
    "Win32_System_IO",
    "Win32_Foundation",
]}

# Performance
rayon = "1.10"                       # parallel computation
compact_str = "0.8"                  # small string optimization

# Utilities
anyhow = "1"                         # error handling
tracing = "0.1"                      # structured logging
tracing-subscriber = "0.3"
bytemuck = "1"                       # safe transmutes for GPU buffers
```

---

## Implementation Order

| Step | Task | Deliverable |
|------|------|-------------|
| 1 | Scaffold project, winit window + vello clear color | Blank GPU window |
| 2 | Implement `jwalk` fallback scanner | FileTree populated from disk |
| 3 | Implement squarified layout algorithm | Vec<LayoutRect> from FileTree |
| 4 | Render flat colored rectangles via Vello | Basic treemap visible |
| 5 | Add cushion gradient shading | Depth-aware 3D appearance |
| 6 | File-type color mapping | Color by extension category |
| 7 | Hover detection + tooltip | Interactive file info |
| 8 | Click to drill down / navigate up | Hierarchical navigation |
| 9 | MFT scanner (admin mode) | Fast NTFS scanning |
| 10 | Polish: animations, LOD, resize | Production quality |

---

## Design Decisions & Rationale

**Why Vello over egui/iced?**
Vello is a low-level scene renderer — it gives us full control over every
pixel. egui/iced are widget toolkits that would fight against custom treemap
rendering. We use Vello for the treemap canvas and can layer simple UI
elements (text, panels) on top.

**Why arena tree over `petgraph` or pointer-based trees?**
A flat `Vec<FileNode>` with index-based links gives us: contiguous memory
layout for cache efficiency, trivial serialization, and O(1) node access.
With 10M+ nodes, this matters.

**Why gradient-approximated cushions first?**
True per-pixel cushion shading requires a custom WGSL shader, which adds
complexity. Vello's built-in gradient fills can approximate the effect well
enough for MVP, letting us iterate on layout and interaction first.

**Why `jwalk` fallback before MFT?**
MFT parsing requires admin privileges and is NTFS-specific. The `jwalk`
path works on any filesystem and any privilege level, giving us a working
tool immediately. MFT is an optimization we layer on top.
